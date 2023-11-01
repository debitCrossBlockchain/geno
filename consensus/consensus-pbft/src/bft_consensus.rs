use ::utils::timer_manager::TimerSender;
use configure::CONFIGURE_INSTANCE_REF;
use consensus_store::bft_storage::BftStorage;
use executor::{block_result::BlockResult, BlockExecutor, LAST_COMMITTED_BLOCK_INFO_REF};
use ledger_upgrade::ledger_upgrade::LedgerUpgradeInstance;
use msp::bytes_to_hex_str;
use network::PeerNetwork;
use parking_lot::RwLock;
use protobuf::error;
use protos::{
    common::{TransactionResult, ValidatorSet},
    consensus::{BftMessageType, BftProof, BftSign, LedgerUpgrade, NewViewRepondParam, TxHashList},
    ledger::{ExtendedData, Ledger},
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tracing::{error, info, Span};
use tx_pool::{
    types::{CommitNotificationSender, TxPoolCommitNotification, TxPoolCommitted},
    TX_POOL_INSTANCE_REF,
};
use utils::{
    general::{hash_crypto_byte, LEDGER_VERSION, MILLI_UNITS_PER_SEC},
    parse::ProtocolParser,
    timer_manager::{TimerEventType, TimerManager},
};

use crate::{
    bft_check_value::{CheckValue, CheckValueResult},
    bft_log::BftLog,
    bft_state::{BftInstancePhase, BftState},
    handler::{
        handler_new_view::HandlerNewView, handler_view_change::HandlerViewChange, BftHandler,
    },
    instance::{bft_instance::BftInstance, bft_instance_index::BftInstanceIndex},
    new_bft_message::NewBftMessage,
};

pub struct BftConsensus {
    pub(crate) logs: BftLog,
    pub(crate) state: BftState,
    pub(crate) timer_sender: TimerSender,
    pub(crate) network_tx: PeerNetwork,
    pub(crate) network_consensus: PeerNetwork,
    pub(crate) ledger_upgrade_instance: Arc<RwLock<LedgerUpgradeInstance>>,
    pub(crate) start_consensus_timer_id: i64,
    pub(crate) ledgerclose_check_timer_id: i64,
    pub(crate) consensus_check_timer_id: i64,
    pub(crate) new_view_repond_timer_id: i64,
    pub(crate) commit_to_txpool_sender: CommitNotificationSender,
    pub(crate) ws_publish_event_sender:
        tokio::sync::mpsc::UnboundedSender<(Ledger, Vec<TransactionResult>)>,
    pub(crate) last_commit_txs: HashMap<String, TxPoolCommitted>,
}

impl BftConsensus {
    pub(crate) fn new(
        timer_sender: TimerSender,
        validators_set: &ValidatorSet,
        last_seq: u64,
        ledger_upgrade_instance: Arc<RwLock<LedgerUpgradeInstance>>,
        network_tx: PeerNetwork,
        network_consensus: PeerNetwork,
        commit_to_txpool_sender: CommitNotificationSender,
        ws_publish_event_sender: tokio::sync::mpsc::UnboundedSender<(
            Ledger,
            Vec<TransactionResult>,
        )>,
    ) -> Self {
        if last_seq >= 2 {
            let validators =
                BftStorage::load_validators(last_seq).expect("validator load failed {}");
            let validators = match validators {
                Some(validators) => validators,
                None => {
                    panic!("validator load none");
                }
            };

            if hash_crypto_byte(&ProtocolParser::serialize::<ValidatorSet>(&validators))
                != hash_crypto_byte(&ProtocolParser::serialize::<ValidatorSet>(validators_set))
            {
                panic!("validator not match");
            }
        } else {
            BftStorage::store_validators(last_seq, validators_set);
        }

        let view_number = BftStorage::load_view_number().unwrap_or(0);
        let mut state = BftState::new(last_seq, network_consensus.clone());
        state.view_number = view_number;
        state.update_validators(validators_set);

        BftConsensus {
            logs: BftLog::default(),
            state,
            timer_sender,
            network_tx,
            network_consensus,
            ledger_upgrade_instance,
            commit_to_txpool_sender,
            start_consensus_timer_id: 0,
            ledgerclose_check_timer_id: 0,
            consensus_check_timer_id: 0,
            new_view_repond_timer_id: 0,
            last_commit_txs: HashMap::default(),
            ws_publish_event_sender,
        }
    }

    pub fn span(&self) -> &Span {
        self.state.span()
    }

    pub fn set_view_number(&mut self, view: i64) {
        self.state.view_number = view;
    }

    pub fn view_number(&self) -> i64 {
        self.state.view_number
    }

    pub fn is_validator(&self) -> bool {
        self.state.is_validator()
    }

    pub fn is_primary(&self) -> bool {
        self.state.is_primary()
    }

    pub fn quorum_size(&self) -> usize {
        self.state.quorum_size()
    }

    pub fn replica_id(&self) -> i64 {
        self.state.replica_id
    }

    pub fn set_last_exe_sequence(&mut self, sequence: u64) {
        if sequence > 0 {
            info!(parent:self.span(),
                "Setting the last execution sequence({})",
                sequence
            );
            self.state.last_exe_sequence = sequence;
        }
    }

    pub fn last_exe_sequence(&self) -> u64 {
        self.state.last_exe_sequence
    }

    pub fn inc_last_exe_sequence(&mut self) {
        self.state.last_exe_sequence += 1;
    }

    pub fn set_view_active(&mut self, is_active: bool) {
        self.state.view_active = is_active;
    }

    pub fn ckp_interval(&self) -> u64 {
        self.state.ckp_interval
    }

    pub fn fault_number(&self) -> u64 {
        self.state.fault_number
    }

    pub fn view_active(&self) -> bool {
        self.state.view_active
    }

    pub fn update_validators(
        &mut self,
        height: u64,
        validators: &ValidatorSet,
        proof: Option<BftProof>,
    ) -> bool {
        let mut new_view_number = -1;
        let mut new_sequence = 0;
        if let Some(proof) = proof {
            //Compare view number
            if proof.get_commits().len() > 0 {
                if let Some(bft_sign) = proof.get_commits().get(0) {
                    let bft = bft_sign.get_bft();
                    let commit = bft.get_commit();
                    if commit.get_base().get_view_number() >= self.view_number() {
                        new_view_number = commit.get_base().get_view_number() + 1;
                    }
                    if commit.get_base().get_sequence() > self.last_exe_sequence() {
                        new_sequence = commit.get_base().get_sequence();
                    }
                }
            }
        } else {
            new_view_number = 0;
        }

        //Compare the validators
        let validator_changed = self.state.validators_change(validators);
        if validator_changed {
            //clear not committed instances
            self.logs.instances.retain(|_, value| {
                (value.phase.clone() as i64) >= (BftInstancePhase::COMMITTED as i64)
            });
            //Clear abnormal records
            self.logs.abnormal_records.clear();

            self.start_ledgerclose_check_timer();
        }

        self.set_last_exe_sequence(new_sequence);

        let mut node_log = String::new();
        if new_view_number > 0 || new_sequence > 0 {
            //clear not committed instances
            self.logs.instances.retain(|_, value| {
                (value.phase.clone() as i64) >= (BftInstancePhase::COMMITTED as i64)
            });

            //Enter to new view
            if new_view_number > 0 {
                self.set_view_number(new_view_number);
            }

            self.set_view_active(true);

            if self.replica_id() >= 0 {
                if self.is_primary() {
                    node_log.push_str("Primary");
                } else {
                    node_log.push_str("Replica");
                }
            } else {
                node_log.push_str("SynNode");
            }

            info!(parent:self.span(),
                "{:?} enter the new view(number:{})",
                node_log,
                self.view_number()
            );
            BftStorage::store_view_number(self.view_number());
            // store_validators
            BftStorage::store_validators(height, validators);
            if height > 3 {
                let _ = BftStorage::delete_validators(height - 3);
            }
            //Delete other incomplete view change instances or other view change instances whose sequence is less than 5.
            self.clear_view_changes();
            self.start_ledgerclose_check_timer();
        }
        return true;
    }

    pub fn clear_view_changes(&mut self) {
        //Delete other incomplete view change instances
        let view_number = self.view_number();
        self.logs.vc_instances.retain(|key, vc_instance| {
            if vc_instance.end_time == 0 {
                // info!(parent:&self.span(),"Delete the view change instance (vn:{}) that is not completed", vc_instance.view_number);
                return false;
            } else if vc_instance.view_number < (view_number - 5) {
                // info!(parent:&self.span(),"Delete the view change instance (vn:{}) that has passed by 5 view.", vc_instance.view_number);
                return false;
            } else {
                return true;
            }
        });
        // BftStorage::store_vc_instances(&self.logs.vc_instances);
    }

    pub fn start_ledgerclose_check_timer(&mut self) {
        TimerManager::instance().delete_timer(self.ledgerclose_check_timer_id);
        self.ledgerclose_check_timer_id = TimerManager::instance().new_delay_timer(
            chrono::Duration::seconds(80),
            self.timer_sender.clone(),
            TimerEventType::PbftLedgerCloseCheck,
            None,
        );
    }

    pub fn start_consensus_publish_timer(&mut self, delay_milliseconds: i64) {
        TimerManager::instance().delete_timer(self.start_consensus_timer_id);
        self.start_consensus_timer_id = TimerManager::instance().new_delay_timer(
            chrono::Duration::milliseconds(delay_milliseconds),
            self.timer_sender.clone(),
            TimerEventType::PbftConsensusPublish,
            None,
        );
    }

    pub fn start_consensus_check_timer(&mut self) {
        self.consensus_check_timer_id = TimerManager::instance().new_repeating_timer(
            chrono::Duration::milliseconds(500),
            self.timer_sender.clone(),
            TimerEventType::PbftConsensusCheck,
            None,
        );
    }

    pub fn start_new_view_repond_timer(&mut self, data: NewViewRepondParam) {
        let data_bytes = ProtocolParser::serialize::<NewViewRepondParam>(&data);
        self.new_view_repond_timer_id = TimerManager::instance().new_delay_timer(
            chrono::Duration::milliseconds(30 * MILLI_UNITS_PER_SEC),
            self.timer_sender.clone(),
            TimerEventType::PbftNewViewRepond,
            Some(data_bytes),
        );
    }

    pub fn delete_new_view_repond_timer(&self) {
        let ret = TimerManager::instance().delete_timer(self.new_view_repond_timer_id);
    }

    pub fn handle_view_changed(&mut self, last_value: &Option<Ledger>) {
        info!(parent:self.span(),"trace-consensus On view changed, will to publish");
        self.publish(last_value);
        self.start_ledgerclose_check_timer();
    }

    pub fn execute_value(&mut self, bft_sign: &BftSign) -> bool {
        let mut exe_vec = Vec::new();
        for (index, instance) in self.logs.instances() {
            if index.sequence <= self.last_exe_sequence() {
                continue;
            }

            if index.sequence == (self.last_exe_sequence() + 1)
                && (instance.phase.clone() as i64) >= (BftInstancePhase::COMMITTED as i64)
            {
                self.inc_last_exe_sequence();
            } else {
                break;
            }

            //Get 'commit' env from buf
            let vec =
                instance.get_bft_sign_vec(&BftInstancePhase::as_phase(&BftMessageType::COMMIT));
            let proof = NewBftMessage::get_commited_proof(vec);

            self.handle_value_committed(index.sequence, instance.pre_prepare.get_value(), proof);

            // self.handle_value_committed_before(
            //     index.sequence,
            //     instance.pre_prepare.get_value(),
            //     proof,
            //     instance.pre_prepare.get_proposer(),
            // );

            exe_vec.push(index.sequence);
        }

        let v = self.ckp_interval() / 2;
        for sequence in exe_vec {
            //Delete the old check point
            if sequence >= v {
                self.logs
                    .instances
                    .retain(|key, value| (key.sequence > (sequence - v)));
            }
        }
        return true;
    }

    pub fn handle_value_committed(&mut self, request_seq: u64, value: &[u8], proof: BftProof) {
        let mut block = match ProtocolParser::deserialize::<Ledger>(value) {
            Ok(block) => block,
            Err(e) => {
                error!(parent:self.span(),"handle_value_committed deserialize Ledger error {}",e);
                return;
            }
        };

        let t0 = chrono::Local::now().timestamp_millis();

        let lcl = { LAST_COMMITTED_BLOCK_INFO_REF.read().get_header().clone() };
        let hash_list = match BlockExecutor::extract_tx_hash_list(&block) {
            Some(value) => match ProtocolParser::deserialize::<TxHashList>(&value) {
                Ok(hash_list) => hash_list,
                Err(e) => {
                    error!(parent:self.span(),"handle_value_committed deserialize TxHashList error {}",e);
                    return;
                }
            },
            None => TxHashList::default(),
        };

        // get tx from tx-pool
        if hash_list.get_hash_set().len() > 0 {
            let (tx_list, lacktx_hash_set) = TX_POOL_INSTANCE_REF
                .read()
                .get_block_by_hashs(hash_list.get_hash_set());
            if lacktx_hash_set.len() > 0 {
                for (hash, index) in lacktx_hash_set.iter() {
                    error!(parent:self.span(),"lacktx hash({}) index({}) in block({})",bytes_to_hex_str(hash),index,block.get_header().get_height());
                }
                return;
            } else {
                let value: Vec<_> = tx_list.iter().map(|t| t.convert_into()).collect();
                block.set_transaction_signs(protobuf::RepeatedField::from(value));
            }
        }

        // add current proof into block
        BlockExecutor::inject_current_proof(
            &mut block,
            ProtocolParser::serialize::<BftProof>(&proof),
        );

        //ledger execute
        let t1 = chrono::Local::now().timestamp_millis();
        match BlockExecutor::execute_block(&block) {
            Ok((tx_list, block_result)) => {
                if let Err(e) = BlockExecutor::commit_block(&mut block, tx_list, &block_result) {
                    error!(parent:self.span(),"handle_value_committed commit_block error {}",e);
                    return;
                } else {
                    // publish event
                    self.send_to_ws(&block, &block_result);
                }
            }
            Err(e) => {
                error!(parent:self.span(),"handle_value_committed execute_block error {}",e);
                return;
            }
        };

        let t2 = chrono::Local::now().timestamp_millis();

        self.delete_commit_tx(&block);

        // utils::transaction_verify_pool::tx_verify_pool_banch_del(bft_value.get_tx_set().get_txs());

        //update_validators
        let validator_set = {
            LAST_COMMITTED_BLOCK_INFO_REF
                .read()
                .get_validators()
                .clone()
        };
        let proof = { LAST_COMMITTED_BLOCK_INFO_REF.read().get_proof() };
        self.update_validators(block.get_header().get_height(), &validator_set, proof);

        let validators: Vec<_> = validator_set
            .get_validators()
            .iter()
            .map(|x| x.get_address().to_string())
            .collect();
        self.network_consensus.update_validators(&validators);
        self.network_tx.update_validators(&validators);

        if self.is_validator() {
            self.ledger_upgrade_instance.write().set_is_validator(true);
        } else {
            self.ledger_upgrade_instance.write().set_is_validator(false);
        }

        {
            self.ledger_upgrade_instance
                .write()
                .set_last_ledger_version(lcl.get_version())
        };

        //start publish timer
        let next_interval = CONFIGURE_INSTANCE_REF.consensus.commit_interval;
        let next_timestamp = next_interval + block.get_header().get_timestamp();
        let block_height = block.get_header().get_height();

        let mut waiting_time = next_timestamp - chrono::Local::now().timestamp_millis();
        if waiting_time <= 0 {
            waiting_time = 1;
        }
        if self.is_primary() {
            self.start_consensus_publish_timer(waiting_time);

            info!(parent:self.span(),
                "Ledger({}) closed successfully,txs({}) txpool commit({})ms, ledger time used ({})ms, next consensus in({})ms",
                block_height,
                block.get_transaction_signs().len(),
                t1 - t0,
                t2 - t1,
                waiting_time,
            );
        } else {
            info!(parent:self.span(),
                "Ledger({}) closed successfully,txs({}) txpool commit({})ms, ledger time used ({})ms, next consensus checked in({})ms",
                block_height,
                block.get_transaction_signs().len(),
                t1 - t0,
                t2 - t1,
                waiting_time
            );
        }

        self.start_ledgerclose_check_timer();
    }

    pub fn send_to_ws(&mut self, block: &Ledger, result: &BlockResult) {
        if let Err(e) = self
            .ws_publish_event_sender
            .send((block.clone(), result.tx_result_set.clone()))
        {
            error!("ws_publish_event_sender send error:{:?}", e);
        }
    }

    pub fn delete_commit_tx(&mut self, block: &Ledger) {
        if block.get_transaction_signs().len() > 0 {
            let mut count = 0;
            let mut transactions: HashMap<String, TxPoolCommitted> = HashMap::new();
            for tx in block.get_transaction_signs().iter() {
                count += 1;
                if let Some(v) = transactions.get_mut(tx.get_transaction().get_source()) {
                    if v.max_seq < tx.get_transaction().get_nonce() {
                        v.max_seq = tx.get_transaction().get_nonce();
                    }
                    v.seqs.insert(tx.get_transaction().get_nonce());
                } else {
                    let sender = tx.get_transaction().get_source().to_string();
                    let mut seqs = HashSet::default();
                    seqs.insert(tx.get_transaction().get_nonce());
                    let c = TxPoolCommitted {
                        sender: sender.clone(),
                        max_seq: tx.get_transaction().get_nonce(),
                        seqs,
                    };
                    transactions.insert(sender, c);
                }
            }
            self.last_commit_txs.clear();
            self.last_commit_txs.clone_from(&transactions);
            info!(parent:self.span(),
                "handle_value_committed ledger({}) txpool commit({}) size",
                block.get_header().get_height(),
                transactions.len()
            );
            let notify = TxPoolCommitNotification {
                transactions,
                count,
            };
            match self.commit_to_txpool_sender.try_send(notify) {
                Err(e) => {
                    error!(parent:self.span(),"handle_value_committed to_tx_pool_commit_sender send error({})",e);
                }
                _ => {}
            }
        }
    }

    pub fn publish(&mut self, last_value: &Option<Ledger>) -> bool {
        if !self.view_active() {
            return true;
        }
        if !self.is_primary() {
            return true;
        }

        info!(parent:self.span(),"Start publish bft,the current node is the leader node and starting consensus processing.");
        let lcl = { LAST_COMMITTED_BLOCK_INFO_REF.read().get_header().clone() };
        let mut next_commit_time = chrono::Local::now().timestamp_millis();
        if next_commit_time < lcl.get_timestamp() + CONFIGURE_INSTANCE_REF.consensus.commit_interval
        {
            next_commit_time =
                lcl.get_timestamp() + CONFIGURE_INSTANCE_REF.consensus.commit_interval
        }

        if let Some(value) = last_value {
            let consensus_value_hash =
                hash_crypto_byte(&ProtocolParser::serialize::<Ledger>(value));
            // let consensus_value_hash = match consensus_value_hash {
            //     Some(value) => value,
            //     None => {
            //         error!(parent:self.span(),"No consensus_value_hash in value");
            //         return false;
            //     }
            // };
            info!(parent:self.span(),
                "The last PREPARED message value is not empty. consensus value digest({})",
                bytes_to_hex_str(&consensus_value_hash)
            );
            if CheckValue::check_value(value, self.span()) == CheckValueResult::Valid {
                info!(parent:self.span(),"Take the last consensus value as the proposal. The number of transactions in consensus value is {}, and the last closed consensus value's hash is {}.", 
                value.get_transaction_signs().len(),
                bytes_to_hex_str(lcl.get_hash()));
                return self.proposal(value);
            }
        }

        let previous_proof = { LAST_COMMITTED_BLOCK_INFO_REF.read().get_proof() };
        let previous_proof_data = match previous_proof {
            Some(proof) => Some(ProtocolParser::serialize::<BftProof>(&proof)),
            None => None,
        };

        //Check whether we need to upgrade the ledger.
        let (validators_set, quorum_size) = self.validators_set_and_quorum();
        let mut new_ledger_version = lcl.get_version();
        if let Some(up) = self.get_ledger_upgrade(&validators_set, quorum_size + 1) {
            info!(parent:self.span(),"Get the upgrade information of the validation node(new_ledger_version:{} successfully.",up.get_new_version());

            if lcl.get_version() < up.get_new_version() && up.get_new_version() <= LEDGER_VERSION {
                new_ledger_version = up.get_new_version();
            }
        };

        let hash_list = {
            TX_POOL_INSTANCE_REF.read().get_block_hash_list(
                CONFIGURE_INSTANCE_REF.consensus.block_max_tx_size,
                CONFIGURE_INSTANCE_REF.consensus.block_max_contract_size,
                &self.last_commit_txs,
            )
        };

        let tx_count = hash_list.len() as u64;
        let total_tx_count = lcl.get_total_tx_count() + tx_count;
        let tx_hash_list = if tx_count > 0 {
            let mut proto_hash_list = TxHashList::default();
            proto_hash_list.set_hash_set(protobuf::RepeatedField::from(hash_list));
            Some(ProtocolParser::serialize::<TxHashList>(&proto_hash_list))
        } else {
            None
        };

        let propose_value = BlockExecutor::initialize_new_block(
            lcl.get_height() + 1,
            Vec::from(lcl.get_hash()),
            next_commit_time,
            new_ledger_version,
            tx_count,
            total_tx_count,
            self.state.node_address.clone(),
            previous_proof_data,
            tx_hash_list,
        );

        return self.proposal(&propose_value);
    }

    fn proposal(&mut self, value: &Ledger) -> bool {
        if !self.is_primary() {
            return false;
        }

        info!(parent:self.span(),
            "Start to request value({})",
            NewBftMessage::consensus_value_desc(&ProtocolParser::serialize::<Ledger>(value))
        );
        if !self.view_active() {
            info!(parent:self.span(),
                "The view-number: {} is not active, so request failed.",
                self.view_number()
            );
            return false;
        }

        //Delete the last uncommitted logs
        let last_exe_seququece = self.last_exe_sequence();
        self.logs.instances.retain(|key, value| {
            !((key.sequence > last_exe_seququece)
                && ((value.phase.clone() as i64) < (BftInstancePhase::COMMITTED as i64)))
        });

        let next_sequence = last_exe_seququece + 1;
        let bft_sign = NewBftMessage::new_pre_prepare(
            &self.state,
            &ProtocolParser::serialize::<Ledger>(value),
            next_sequence,
        );

        //Check the index
        let index = BftInstanceIndex::new(self.view_number(), next_sequence);
        //Insert the instance to map
        let mut instance = BftInstance::new();
        instance.pre_prepare_msg.clone_from(&bft_sign);
        instance.phase.clone_from(&BftInstancePhase::PRE_PREPARED);
        instance
            .pre_prepare
            .clone_from(bft_sign.get_bft().get_pre_prepare());
        let phase = BftInstancePhase::as_phase(&bft_sign.get_bft().get_msg_type());
        instance.update_msg_buf(&phase, &bft_sign);
        self.logs.instances.insert(index, instance);

        info!(parent:self.span(),
            "Send pre-prepare message: view number({}), sequence({}), consensus value({}) value_digest({})",
            self.view_number(),
            next_sequence,
            NewBftMessage::consensus_value_desc(&ProtocolParser::serialize::<Ledger>(value)),
            bytes_to_hex_str(bft_sign.get_bft().get_pre_prepare().get_value_digest())
        );

        //Broadcast the message to other nodes
        self.state.broadcast_message(&bft_sign);
        true
    }

    pub fn validators_set_and_quorum(&self) -> (ValidatorSet, usize) {
        (
            self.state.validators.validators_set(),
            self.state.quorum_size(),
        )
    }

    pub fn get_ledger_upgrade(
        &self,
        validators_set: &ValidatorSet,
        quorum_size: usize,
    ) -> Option<LedgerUpgrade> {
        self.ledger_upgrade_instance
            .write()
            .get_valid(validators_set, quorum_size)
    }

    pub fn check_consensus_timeout(&mut self, current_time: i64) {
        self.logs
            .check_instances_timeout(&mut self.state, current_time);
        self.logs
            .check_vc_instances_timeout(&mut self.state, current_time);
    }

    pub fn start_view_change(&mut self) {
        info!(parent:self.span(),"trace-consensus ledger close timeout");
        self.state.start_view_change(&self.logs.instances);
    }

    pub fn handle_new_view_repond_timer(&mut self, data: &[u8]) {
        match ProtocolParser::deserialize::<NewViewRepondParam>(data) {
            Ok(p) => {
                if self.view_active() {
                    info!(parent:self.span(),
                        "The current view({}) is active, so do not send new view(vn:{})",
                        self.view_number(),
                        p.get_view_number()
                    );
                } else {
                    info!(parent:self.span(),"The new view(vn:{})'s primary was not respond,  then negotiates next view(vn:{})", p.get_view_number()
						, p.get_view_number() + 1);

                    let msg = NewBftMessage::new_view_change_raw_value(
                        &self.state,
                        p.get_view_number() + 1,
                        p.get_prepared_set(),
                        &self.logs.instances,
                    );
                    //SEND NEW VIEW
                    info!(parent:self.span(),
                        "trace-consensus Sending view change message again, new view number({}), desc({:?})",
                        p.get_view_number() + 1,NewBftMessage::bft_desc(msg.get_bft())
                    );
                    self.state.broadcast_message(&msg);
                }
            }
            Err(e) => {
                error!(parent:self.span(),
                    "Failed to process the NewViewRepondParam message,err {}",
                    e
                );
            }
        }
    }

    pub fn handle_receive_consensus(&mut self, bft_sign: &BftSign) -> bool {
        if !self.is_validator() {
            return true;
        }
        let bft = bft_sign.get_bft();
        //Check the message item.
        if !self.state.check_bft_message(&bft_sign) {
            return false;
        }
        let mut ret = false;
        match bft.get_msg_type() {
            BftMessageType::PRE_PREPARE | BftMessageType::PREPARE | BftMessageType::COMMIT => {
                let mut check_value = CheckValueResult::Valid;
                if bft.get_msg_type() == BftMessageType::PRE_PREPARE {
                    check_value = CheckValue::check_value_bytes(
                        bft.get_pre_prepare().get_value(),
                        self.span(),
                    );
                }
                if !BftHandler::create_instance(self, bft_sign) {
                    return false;
                };

                let mut trigger_committed = false;
                let ret = self.logs.handle_instance(
                    bft_sign,
                    &mut self.state,
                    check_value,
                    &mut trigger_committed,
                );
                if trigger_committed {
                    self.execute_value(bft_sign);
                }
            }
            BftMessageType::VIEW_CHANGE_VALUE => {
                ret = HandlerViewChange::handler_view_change_value(self, &bft_sign);
            }
            BftMessageType::NEW_VIEW => {
                ret = HandlerNewView::handler_new_view(self, &bft_sign);
            }
            _ => {
                ret = false;
            }
        }

        ret
    }
}
