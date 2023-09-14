use crate::bft_consensus::BftConsensus;
use crate::bft_state::BftInstancePhase::{COMMITTED, PREPARED};

use crate::instance::bft_vc_instance::BftVcInstance;
use crate::new_bft_message::NewBftMessage;

use consensus_store::bft_storage::BftStorage;
use protobuf::Message;
use protos::consensus::{BftSign, NewViewRepondParam};
use protos::ledger::Ledger;
use tracing::*;
use utils::parse::ProtocolParser;

pub struct HandlerViewChange {}

impl HandlerViewChange {
    pub fn handler_view_change_value(bft_consensus: &mut BftConsensus, bft_sign: &BftSign) -> bool {
        let bft = bft_sign.get_bft();
        let view_change_raw = bft.get_view_change_value();
        let inner_bft_env = view_change_raw.get_view_change_env();
        let view_change = inner_bft_env.get_bft().get_view_change();

        info!(parent:bft_consensus.span(),
            "handle_quorum_view_change Received view change message from {},round number({})",
            NewBftMessage::base_info_desc(view_change.get_base()),
            bft.get_round_number()
        );

        if view_change.get_base().get_view_number() == bft_consensus.view_number() {
            info!(parent:bft_consensus.span(),
                "handle_quorum_view_change The new view number({}) is equal to current view number, then do nothing",
                view_change.get_base().get_view_number()
            );
            return true;
        } else if view_change.get_base().get_view_number() < bft_consensus.view_number() {
            info!(parent:bft_consensus.span(),
                "handle_quorum_view_change The new view number({}) is less than current view number({}), then do nothing",
                view_change.get_base().get_view_number(),
                bft_consensus.view_number()
            );
            return false;
        }

        if !bft_consensus
            .logs
            .vc_instances
            .contains_key(&view_change.get_base().get_view_number())
        {
            let mut vc_instance = BftVcInstance::new();
            vc_instance.set_view_number(view_change.get_base().get_view_number());
            vc_instance.set_sequence(view_change.get_base().get_sequence());
            bft_consensus
                .logs
                .vc_instances
                .insert(view_change.get_base().get_view_number(), vc_instance);
            info!(parent:bft_consensus.span(),
                "handle_quorum_view_change insert view number({}), sequence({}) vc_instance",
                view_change.get_base().get_view_number(),
                view_change.get_base().get_sequence(),
            );
        }

        let span = bft_consensus.state.span();
        let replica_id = bft_consensus.replica_id();
        let last_exe_seq = bft_consensus.last_exe_sequence();
        let quorum_size = bft_consensus.quorum_size();
        if let Some(vc_instance) = bft_consensus
            .logs
            .vc_instances
            .get_mut(&view_change.get_base().get_view_number())
        {
            //Insert into the msg need to be sent again for timeout
            if (view_change.get_base().get_replica_id() == replica_id)
                && (!vc_instance.view_change_msg.has_bft())
            {
                vc_instance.view_change_msg.clone_from(bft_sign);
                info!(
                    parent: span,
                    "handle_quorum_view_change insert vc_instance: view number({})",
                    view_change.get_base().get_view_number(),
                );
            }

            if !vc_instance
                .view_changes
                .contains_key(&view_change.get_base().get_replica_id())
            {
                vc_instance.msg_buf.push(bft_sign.clone());
                vc_instance
                    .view_changes
                    .insert(view_change.get_base().get_replica_id(), view_change.clone());
                info!(
                    parent: span,
                    "handle_quorum_view_change insert view_change: replica_id({})",
                    view_change.get_base().get_replica_id(),
                );
            }
            info!(
                parent: span,
                "handle_quorum_view_change view_change_raw has_prepared_set({})",
                view_change_raw.has_prepared_set()
            );
            if view_change_raw.has_prepared_set() {
                let pre_prepared_pbft = view_change_raw.get_prepared_set().get_pre_prepare();
                let last_pre_prepared_pbft = vc_instance.pre_prepared_env_set.get_pre_prepare();

                let msg_seq = pre_prepared_pbft
                    .get_bft()
                    .get_pre_prepare()
                    .get_base()
                    .get_sequence();
                let last_seq = last_pre_prepared_pbft
                    .get_bft()
                    .get_pre_prepare()
                    .get_base()
                    .get_sequence();
                if (msg_seq > last_seq) && (msg_seq > last_exe_seq) {
                    info!(parent:span,
                        "handle_quorum_view_change Replacing the view-change instance's pre-prepared env, pbft desc({})",
                        NewBftMessage::bft_desc(pre_prepared_pbft.get_bft())
                    );
                    vc_instance
                        .pre_prepared_env_set
                        .clone_from(view_change_raw.get_prepared_set());
                }
            }

            info!(parent:span,
                "handle_quorum_view_change view_changes size({}) quorum_size({}) vc_instance.end_time({})",
                vc_instance.view_changes.len() , quorum_size,vc_instance.end_time
            );

            if (vc_instance.view_changes.len() > quorum_size) && (vc_instance.end_time == 0) {
                //for view change, quorum size is 2f
                //View changes have achieved
                info!(
                    parent: span,
                    "handle_quorum_view_change Process quorum view-change, new view (number:{})",
                    vc_instance.view_number
                );
                //Insert into the msg need to be sent again for timeout
                let ret = Self::handle_quorum_view_change(bft_consensus, bft_sign);
                // BftStorage::store_vc_instances(&bft_consensus.logs.vc_instances);
                return ret;
            }
        }
        true
    }

    pub fn handle_quorum_view_change(bft_consensus: &mut BftConsensus, bft_sign: &BftSign) -> bool {
        let bft = bft_sign.get_bft();
        let view_change = bft
            .get_view_change_value()
            .get_view_change_env()
            .get_bft()
            .get_view_change();

        let validators_size = bft_consensus.state.validators.len() as i64;
        let view_number = bft_consensus.view_number();
        info!(parent:bft_consensus.span(),
            "handle_quorum_view_change Process quorum view-change, new view (number:{})",
            view_change.get_base().get_view_number()
        );

        for (abnormal_node, replica_id) in bft_consensus.state.validators.iter() {
            if replica_id == (view_number % validators_size) {
                if !bft_consensus
                    .logs
                    .abnormal_records
                    .contains_key(abnormal_node.as_str())
                {
                    bft_consensus.logs.abnormal_records.insert(abnormal_node, 1);
                }
                break;
            }
        }

        let replica_id = bft_consensus.replica_id();
        let last_exe_seq = bft_consensus.last_exe_sequence();
        // let view_active = bft_consensus.view_active();

        let mut last_consensus_value = Ledger::new();
        let mut instance_view_number = 0;
        if let Some(vc_instance) = bft_consensus
            .logs
            .vc_instances
            .get_mut(&view_change.get_base().get_view_number())
        {
            instance_view_number = vc_instance.view_number;
            // we must be the leader
            if (instance_view_number % (validators_size as i64)) != replica_id {
                let temp_set = vc_instance.pre_prepared_env_set.clone();
                let mut param = NewViewRepondParam::new();
                param.set_view_number(instance_view_number);
                param.set_prepared_set(temp_set);
                bft_consensus.start_new_view_repond_timer(param);

                info!(parent:bft_consensus.state.span(),"handle_quorum_view_change It's not the new primary(replica_id:{}), so don't process the quorum view message, waiting for new view message 30s",
                instance_view_number % (validators_size as i64));
                return true;
            }

            //New view message
            let new_view = NewBftMessage::new_new_view(&bft_consensus.state, vc_instance);
            // info!(parent:span,
            //     "handle_quorum_view_change Send new view(replica_id({}),view_number({}),sequence({}))",
            //     new_view.get_bft().get_new_view().get_replica_id(),
            //     new_view.get_bft().get_new_view().get_view_number(),
            //     new_view.get_bft().get_new_view().get_sequence()
            // );
            //Send new view message
            vc_instance.send_new_view(
                &mut bft_consensus.state,
                new_view,
                chrono::Local::now().timestamp_millis(),
            );

            //Get last prepared consensus value
            if vc_instance.pre_prepared_env_set.has_pre_prepare() {
                let bft = vc_instance.pre_prepared_env_set.get_pre_prepare().get_bft();

                match ProtocolParser::deserialize::<Ledger>(bft.get_pre_prepare().get_value()) {
                    Ok(v) => last_consensus_value.clone_from(&v),
                    Err(e) => {
                        // error!(parent:bft_consensus.span(),"handle_quorum_view_change Parse pre-prepared value error:{}",e);
                        error!(
                            "handle_quorum_view_change Parse pre-prepared value error:{}",
                            e
                        );
                    }
                }
            }

            //set end time
            vc_instance.change_complete(chrono::Local::now().timestamp_millis());
        }

        //Delete uncommitted instances
        bft_consensus.logs.instances.retain(|key, value| {
            (value.phase == COMMITTED) || (value.phase == PREPARED) && (key.sequence > last_exe_seq)
        });

        bft_consensus.set_view_number(instance_view_number);
        bft_consensus.set_view_active(true);
        // BftStorage::store_value_i64(VIEW_ACTIVE, 1);

        info!(parent:bft_consensus.span(),
            "handle_quorum_view_change Primary enter the new view number:{}",
            instance_view_number
        );
        BftStorage::store_view_number(instance_view_number);

        // //set end time
        // vc_instance.change_complete(chrono::Local::now().timestamp_millis());

        bft_consensus.clear_view_changes();
        bft_consensus.start_ledgerclose_check_timer();

        // OnViewChanged
        if last_consensus_value.compute_size() != 0 {
            info!(parent:bft_consensus.span(),
                "handle_quorum_view_change trace-consensus handle_view_changed last_consensus_value sequence({})",last_consensus_value.get_header().get_height()
            );
            bft_consensus.handle_view_changed(&Some(last_consensus_value));
        } else {
            info!(parent:bft_consensus.span(),"handle_quorum_view_change trace-consensus handle_view_changed");
            bft_consensus.handle_view_changed(&None);
        }

        true
    }
}
