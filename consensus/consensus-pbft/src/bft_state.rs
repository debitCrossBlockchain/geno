use msp::signing::{eddsa_ed25519::EddsaEd25519PrivateKey, PrivateKey};
use network::{LocalBusPublisher, PeerNetwork, ReturnableProtocolsMessage};
use protos::{
    common::{
        ProtocolsActionMessageType, ProtocolsMessage, ProtocolsMessageType, Signature, ValidatorSet,
    },
    consensus::{BftMessageType, BftPreparedSet, BftSign, Consensus, ConsensusType},
};
use tracing::*;
use utils::{
    general::{node_address, node_private_key, self_chain_hub, self_chain_id},
    parse::ProtocolParser,
    verify_sign::sign,
    LogUtil,
};

use crate::{
    bft_check_value::CheckValue, bft_log::BftInstanceMap, new_bft_message::NewBftMessage,
    utils::quorum_size, validators::Validators,
};

#[derive(Debug, Copy, Clone, Hash)]
pub enum BftInstancePhase {
    NONE = 0,
    PRE_PREPARED = 1,
    PREPARED = 2,
    COMMITTED = 3,
    MAX = 4,
}

//Phase => message type
//Phase           NONE          | PRE-PREPARED     | PREPARED | COMMITTED
//message type    PRE-PREPARE   | PREPARE          | COMMIT   | REPLY
impl BftInstancePhase {
    pub fn as_phase(bft_type: &BftMessageType) -> BftInstancePhase {
        match bft_type {
            BftMessageType::PRE_PREPARE => BftInstancePhase::NONE,
            BftMessageType::PREPARE => BftInstancePhase::PRE_PREPARED,
            BftMessageType::COMMIT => BftInstancePhase::PREPARED,
            _ => BftInstancePhase::MAX,
        }
    }
}

impl PartialEq for BftInstancePhase {
    fn eq(&self, other: &BftInstancePhase) -> bool {
        (self.clone() as i64) == (other.clone() as i64)
    }
}

impl Eq for BftInstancePhase {}

#[derive(Clone)]
pub struct BftState {
    pub(crate) is_validator: bool,
    pub(crate) private_key: EddsaEd25519PrivateKey,
    pub(crate) chain_id: String,
    pub(crate) chain_hub: String,
    pub(crate) node_address: String,
    pub(crate) replica_id: i64,
    pub(crate) validators: Validators,
    //For bft instance
    pub(crate) ckp_interval: u64,
    pub(crate) view_number: i64,
    pub(crate) last_exe_sequence: u64,
    pub(crate) fault_number: u64,
    pub(crate) view_active: bool,
    pub(crate) network: PeerNetwork,
    pub(crate) span: Span,
    pub(crate) publisher: LocalBusPublisher<ProtocolsMessageType, ReturnableProtocolsMessage>,
}

impl BftState {
    pub fn new(last_sequence: u64, network: PeerNetwork) -> Self {
        let publisher = network.publisher();
        let private_key =
            EddsaEd25519PrivateKey::from_hex(&node_private_key()).expect("private key error");
        Self {
            is_validator: false,
            private_key,
            chain_id: self_chain_id(),
            chain_hub: self_chain_hub(),
            node_address: node_address(),
            replica_id: -1,
            validators: Validators::default(),
            view_number: 0,
            ckp_interval: 10,
            last_exe_sequence: last_sequence,
            fault_number: 0,
            view_active: true,
            network,
            span: LogUtil::create_span("consensus"),
            publisher,
        }
    }

    pub fn span(&self) -> &Span {
        &self.span
    }

    pub fn is_validator(&self) -> bool {
        self.is_validator
    }

    pub fn broadcast_message(&self, message: &BftSign) {
        if !self.is_validator() {
            return;
        }
        let mut consensus = Consensus::default();
        consensus.set_consensus_type(ConsensusType::PBFT);
        consensus.set_msg(ProtocolParser::serialize::<BftSign>(message));
        let mut msg = ProtocolsMessage::new();
        msg.set_msg_type(ProtocolsMessageType::CONSENSUS);
        msg.set_action(ProtocolsActionMessageType::BROADCAST);
        msg.set_data(ProtocolParser::serialize::<Consensus>(&consensus));
        self.network.broadcast_msg(msg.clone());
        self.publisher.publish(
            ProtocolsMessageType::CONSENSUS,
            (self.network.listen_endpoint(), msg),
        );
    }

    pub fn is_solo(&self) -> bool {
        if ((self.view_number % self.validators.len() as i64) == self.replica_id)
            && (self.validators.len() != 1)
        {
            return true;
        }
        false
    }

    pub fn quorum_size(&self) -> usize {
        quorum_size(self.validators.len())
    }

    pub fn sign_data(&self, data: &[u8]) -> Signature {
        match sign(&self.private_key.as_hex(), data, "eddsa_ed25519") {
            Ok(sign_ret) => sign_ret,
            Err(e) => {
                error!("sign error:{}", e);
                Signature::default()
            }
        }
    }

    pub fn update_validators(&mut self, validators_set: &ValidatorSet) {
        self.validators.update_validators(&validators_set);

        let node_address = self.private_key.get_address();
        if let Some(replica_id) = self.validators.replica_id(node_address.as_str()) {
            self.is_validator = true;
            self.replica_id = replica_id;
        } else {
            self.is_validator = false;
            self.replica_id = -1;
        }
    }

    pub fn is_primary(&self) -> bool {
        if self.is_validator()
            && ((self.view_number % self.validators.len() as i64) == self.replica_id)
        {
            return true;
        }

        false
    }

    pub fn start_view_change(&mut self, instances: &BftInstanceMap) {
        if !self.is_validator {
            return;
        }

        let null_set: BftPreparedSet = BftPreparedSet::default();
        self.view_active = false;
        let msg = NewBftMessage::new_view_change_raw_value(
            self,
            self.view_number + 1,
            &null_set,
            instances,
        );
        info!(parent:self.span(),
            "trace-consensus Sending view change message, new view number({}), desc({:?})",
            self.view_number + 1,
            NewBftMessage::bft_desc(msg.get_bft())
        );
        self.broadcast_message(&msg);
    }

    pub fn validators_change(&mut self, validators: &ValidatorSet) -> bool {
        let changed = self.validators.changed(validators);
        if changed {
            info!(parent:self.span(),
                "validators change:({}) | ({}) | ({:?})",
                changed,
                self.validators,
                validators.get_validators(),
            );
            //Update the validators
            self.update_validators(validators);
            if self.validators.len() < 4 {
                warn!(
                    "bft couldn't tolerate fault node when validator size = {}.",
                    self.validators.len()
                );
            }

            self.fault_number = ((self.validators.len() - 1) / 3) as u64;
            let mut log_info = String::new();
            if (self.view_number % (self.validators.len() as i64)) == self.replica_id {
                log_info.push_str("is");
            } else {
                log_info.push_str("is not");
            }

            info!(parent:self.span(),"When validator size = {}, bft can tolerate {} fault nodes. Current node's replica_id = {}, so it {} a leader",
                  self.validators.len(), self.fault_number, self.replica_id, log_info);
        }
        changed
    }

    pub fn check_bft_message(&self, env: &BftSign) -> bool {
        CheckValue::check_message(env, &self.validators)
    }
}
