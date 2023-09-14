use crate::{new_bft_message::NewBftMessage, utils::quorum_size, validators::Validators};
use executor::LAST_COMMITTED_BLOCK_INFO_REF;
use protobuf::Message;
use protos::{
    common::ValidatorSet,
    consensus::*,
    ledger::{Ledger, LedgerHeader},
};
use state_store::StateStorage;
use std::collections::HashSet;
use tracing::{error, trace, Span};
use utils::{
    general::{self_chain_hub, self_chain_id},
    parse::ProtocolParser,
    verify_sign::{get_sign_address, verify_sign},
};

#[derive(Debug, Copy, Clone, Hash)]
pub enum CheckValueResult {
    Valid = 0,
    MayValid = 1,
    InValid = 2,
}

impl PartialEq<Self> for CheckValueResult {
    fn eq(&self, other: &Self) -> bool {
        (self.clone() as i64) == (other.clone() as i64)
    }
}

impl Eq for CheckValueResult {}

pub struct CheckValue;

impl CheckValue {
    pub fn check_value_bytes(value: &[u8], span: &Span) -> CheckValueResult {
        let bft_value = match ProtocolParser::deserialize::<Ledger>(value) {
            Ok(v) => v,
            Err(e) => {
                error!(
                    parent: span,
                    "Failed to parse consensus value in check_value {}", e
                );
                return CheckValueResult::MayValid;
            }
        };

        Self::check_value(&bft_value, span)
    }

    pub fn check_value(bft_value: &Ledger, span: &Span) -> CheckValueResult {
        let lcl = { LAST_COMMITTED_BLOCK_INFO_REF.read().get_header().clone() };
        let last_ledger_sequence = lcl.get_height();
        let mut pre_pre_v_set = ValidatorSet::new();
        // current ledger 3, last_ledger_sequence 2, pre_pre_v_set 1
        if last_ledger_sequence > 1 {
            //Get the validator set for the pre pre ledger.
            match StateStorage::get_validators_by_seq(bft_value.get_header().get_height() - 2) {
                Err(e) => {
                    error!(
                        parent: span,
                        "Failed to get validator of ledger ({}) in check_value, error:{}",
                        bft_value.get_header().get_height() - 2,
                        e
                    );
                    return CheckValueResult::MayValid;
                }
                Ok(v) => {
                    if let Some(validators) = v {
                        pre_pre_v_set.clone_from(&validators);
                    } else {
                        error!(
                            parent: span,
                            "Failed to get validator of ledger ({}) in check_value, no validators",
                            bft_value.get_header().get_height() - 2,
                        );
                        return CheckValueResult::MayValid;
                    }
                }
            }
        }

        CheckValue::check_value_help(&bft_value, &lcl, &pre_pre_v_set, span)
    }

    pub fn check_value_help(
        block: &Ledger,
        lcl: &LedgerHeader,
        validators_set: &ValidatorSet,
        span: &Span,
    ) -> CheckValueResult {
        // let value_size = block.compute_size();

        //Check the previous ledger sequence.
        if block.get_header().get_height() != (lcl.get_height() + 1) {
            error!(parent:span,"Previous ledger's sequence {} + 1 is not equal to ledger sequence {} in consensus message.",lcl.get_height(),block.get_header().get_height());
            return CheckValueResult::MayValid;
        }

        //Check the previous hash.
        if block.get_header().get_previous_hash() != lcl.get_hash() {
            error!(parent:span,"Previous ledger {} hash {} in current node is not equal to the previous ledger hash {} in consensus value.",
                   lcl.get_height(),msp::bytes_to_hex_str(lcl.get_hash()),msp::bytes_to_hex_str(block.get_header().get_previous_hash()));
            return CheckValueResult::MayValid;
        }

        //Check whether we need to upgrade the ledger.
        let new_block_version = block.get_header().get_version();
        if new_block_version != lcl.get_version() {
            if lcl.get_version() >= new_block_version {
                error!(parent:span,"New ledger's version({}) is less than or equal to last closed ledger's version({})",
                new_block_version, lcl.get_version());
                return CheckValueResult::MayValid;
            }

            if new_block_version > utils::general::LEDGER_VERSION {
                error!(
                    parent: span,
                    "New ledger's version ({}) is larger than program's version({}).",
                    new_block_version,
                    utils::general::LEDGER_VERSION
                );
                return CheckValueResult::MayValid;
            }
        }

        //Check the second block
        let previous_proof = if let Some(data) = block
            .get_extended_data()
            .get_extra_data()
            .get(utils::general::BFT_PREVIOUS_PROOF)
        {
            let proof = match ProtocolParser::deserialize::<BftProof>(data) {
                Ok(pf) => {
                    if lcl.get_height() == 1 && !pf.get_commits().is_empty() {
                        error!(
                            parent: span,
                            "The second block's previous consensus proof must be empty."
                        );
                        return CheckValueResult::MayValid;
                    }
                    pf
                }
                Err(e) => {
                    error!(parent: span, "{}", e);
                    return CheckValueResult::InValid;
                }
            };
            proof
        } else {
            error!(parent: span, "New ledger no previous consensus proof.",);
            return CheckValueResult::InValid;
        };

        //Check this proof
        if lcl.get_height() > 1 {
            let consensus_value_hash = match block
                .get_header()
                .get_extended_data()
                .get_extra_data()
                .get(utils::general::BFT_CONSENSUS_VALUE_HASH)
            {
                Some(data) => data.clone(),
                None => {
                    error!(parent: span, "New ledger no consensus value hash.",);
                    return CheckValueResult::InValid;
                }
            };
            if !Self::check_proof(
                &validators_set,
                &consensus_value_hash,
                &previous_proof,
                span,
            )
            //&& not_find_hard
            {
                error!(
                    parent: span,
                    "Failed to check the value because the proof is not valid "
                );
                return CheckValueResult::MayValid;
            }
        }
        return CheckValueResult::Valid;
    }

    pub fn check_proof(
        validators: &ValidatorSet,
        previous_hash: &[u8],
        proof: &BftProof,
        span: &Span,
    ) -> bool {
        let mut temp_vs = Validators::default();
        temp_vs.update_validators(validators);

        let total_size = temp_vs.len();
        let q_size = quorum_size(total_size) + 1;

        //Check proof
        for bft_sign in proof.commits.to_vec() {
            let bft = bft_sign.get_bft();
            if !Self::check_message(&bft_sign, &temp_vs) {
                error!(parent: span,"Failed to check proof message item: validators:({:?}), hash ({:?}), proof({:?}), total_size({}), qsize({}), counter({})",
                       validators.get_validators(),String::from_utf8(previous_hash.to_vec()).unwrap(), bft_sign, total_size, q_size, temp_vs.len());
                return false;
            }

            if bft.get_msg_type() != BftMessageType::COMMIT || !bft.has_commit() {
                error!(
                    parent: span,
                    "Failed to check proof message item: type({:?}) is not valid.",
                    bft.get_msg_type()
                );
                return false;
            }

            let commit = bft.get_commit();
            if commit.get_value_digest() != previous_hash {
                error!(parent: span,"Failed to check proof message item, because message value hash {:?} is not equal to previous value hash {:?}",
                       commit.get_value_digest(), previous_hash);
                return false;
            }

            let sign = bft_sign.get_signature();
            let address = match get_sign_address(sign) {
                Ok(address) => address,
                Err(e) => {
                    error!(
                        parent: span,
                        "Failed to check proof message item, because get sign address error: {}", e
                    );
                    return false;
                }
            };
            if !temp_vs.contains(&address) {
                error!(
                    parent: span,
                    "Failed to check proof, because signature({:?}) is not found or duplicated",
                    address
                );
                return false;
            }
            temp_vs.remove(&address);
        }

        return if (total_size - temp_vs.len()) >= q_size {
            true
        } else {
            error!(
                parent: span,
                "Failed to check proof, because message quorum size({}) < quorum size({}) ",
                total_size - temp_vs.len(),
                q_size
            );
            false
        };
    }

    pub fn check_message(bft_sign: &BftSign, validators: &Validators) -> bool {
        //This function should output the error log
        let bft = bft_sign.get_bft();
        let sign = bft_sign.get_signature();

        let address = match get_sign_address(sign) {
            Ok(address) => address,
            Err(e) => {
                error!(
                    "Failed to check proof message item, because get sign address error: {}",
                    e
                );
                return false;
            }
        };

        if bft_sign.get_chain_id() != self_chain_id() {
            trace!(
                "Failed to check same chain, node self id {} is not eq {}",
                bft_sign.get_chain_id(),
                self_chain_id()
            );
            return false;
        }

        if bft_sign.get_chain_hub() != self_chain_hub() {
            trace!(
                "Failed to check same chain hub, node self id {} is not eq {}",
                bft_sign.get_chain_id(),
                self_chain_hub()
            );
            return false;
        }

        //Check the node id to see if it exists in the validator' list
        let real_replica_id = match validators.get(&address) {
            Some(replica_id) => replica_id,
            None => {
                error!("Unable to find validator {} from validators", address);
                return false;
            }
        };

        let mut replica_id = -1;
        if !Self::check_phase(bft_sign, validators, &mut replica_id) {
            return false;
        }

        //Check if replica_id is equal to the object id
        if replica_id != real_replica_id {
            error!("Failed to check the received message (type:{:?}), because the message replica id {} is not equal to the signature id {}",
            NewBftMessage::bft_desc(bft), replica_id, real_replica_id);
            return false;
        }

        //Check the signature

        match verify_sign(sign, &ProtocolParser::serialize(bft)) {
            Ok(ret) => {
                if !ret {
                    error!(
                        "Failed to check received message's signature, desc({})",
                        NewBftMessage::bft_desc(bft)
                    );
                    return false;
                }
            }
            Err(_) => {
                error!("verify_sign error, desc({})", NewBftMessage::bft_desc(bft));
                return false;
            }
        }

        return true;
    }

    fn check_phase(bft_sign: &BftSign, validators: &Validators, replica_id: &mut i64) -> bool {
        let bft = bft_sign.get_bft();
        //Check bft type is no larger than max
        match bft.get_msg_type() {
            BftMessageType::PRE_PREPARE => {
                if !bft.has_pre_prepare() {
                    error!("Pre-Prepare message has no instance.");
                    return false;
                }
                replica_id.clone_from(&bft.get_pre_prepare().get_base().get_replica_id());
            }
            BftMessageType::PREPARE => {
                if !bft.has_prepare() {
                    error!("Prepare message has no instance.");
                    return false;
                }
                replica_id.clone_from(&bft.get_prepare().get_base().get_replica_id());
            }
            BftMessageType::COMMIT => {
                if !bft.has_commit() {
                    error!("Commit message has no instance");
                    return false;
                }
                replica_id.clone_from(&bft.get_commit().get_base().get_replica_id());
            }
            BftMessageType::VIEW_CHANGE => {
                if !bft.has_view_change() {
                    error!("View change message has no instance");
                    return false;
                }
                replica_id.clone_from(&bft.get_view_change().get_base().get_replica_id());
            }
            BftMessageType::VIEW_CHANGE_VALUE => {
                if !bft.has_view_change_value() {
                    error!("View change with raw value message has no instance");
                    return false;
                }

                if !Self::check_view_change_value(bft.get_view_change_value(), validators) {
                    return false;
                }
                let view_change_raw = bft.get_view_change_value().get_view_change_env();
                replica_id.clone_from(
                    &view_change_raw
                        .get_bft()
                        .get_view_change()
                        .get_base()
                        .get_replica_id(),
                );
            }
            BftMessageType::NEW_VIEW => {
                if !bft.has_new_view() {
                    error!("New view message has no instance");
                    return false;
                }
                replica_id.clone_from(&bft.get_new_view().get_base().get_replica_id());
            }
        };
        true
    }

    pub fn check_view_change_value(
        view_change_value: &BftViewChangeValue,
        validators: &Validators,
    ) -> bool {
        if !view_change_value.has_view_change_env() {
            error!(
                "Failed to check raw view-change, there is no view change env, desc {}",
                NewBftMessage::view_change_value_desc(view_change_value)
            );
            return false;
        }

        let view_change_env = view_change_value.get_view_change_env();
        if view_change_env.get_bft().get_msg_type() != BftMessageType::VIEW_CHANGE
            || !Self::check_message(view_change_env, validators)
        {
            error!(
                "Failed to check raw view-change, desc {:?}",
                NewBftMessage::view_change_value_desc(view_change_value)
            );
            return false;
        }

        let p_value_digest = view_change_env
            .get_bft()
            .get_view_change()
            .get_prepared_value_digest();
        let mut value_digest: Vec<u8> = Vec::new();
        if view_change_value.has_prepared_set() {
            //Check the prepared message
            let prepared_set = view_change_value.get_prepared_set();
            //Check the pre-prepared message
            let pre_prepare_env = prepared_set.get_pre_prepare();
            if pre_prepare_env.compute_size() == 0 {
                return true;
            }
            let pre_prepare = pre_prepare_env.get_bft().get_pre_prepare();
            if !Self::check_message(pre_prepare_env, validators) {
                return false;
            }

            value_digest.clone_from(&pre_prepare.value_digest);

            let mut replica_ids: HashSet<i64> = HashSet::new();
            //Check the prepared message
            for prepare_env in prepared_set.get_prepare() {
                if !Self::check_message(prepare_env, validators) {
                    error!(
                        "Failed to check view-change prepared set, desc {}",
                        NewBftMessage::view_change_value_desc(view_change_value)
                    );
                    return false;
                }

                let prepare = prepare_env.get_bft().get_prepare();
                if (prepare.get_base().get_view_number()
                    != pre_prepare.get_base().get_view_number())
                    || (prepare.get_base().get_sequence() != pre_prepare.get_base().get_sequence())
                    || (prepare.get_value_digest() != pre_prepare.get_value_digest())
                {
                    error!(
                        "Failed to check view-change prepared set, desc {}",
                        NewBftMessage::view_change_value_desc(view_change_value)
                    );
                    return false;
                }

                replica_ids.insert(prepare.get_base().get_replica_id());
            }

            let quorum_len = quorum_size(validators.len());
            if replica_ids.len() < quorum_len {
                error!("The raw-view-change message's prepared message's replica number {} is less than quorum size {}", replica_ids.len(), quorum_len + 1);
                return false;
            }
        }

        if p_value_digest != value_digest.as_slice() {
            error!(
                "Failed to check view-change, because inner value digest is difference, desc {}",
                NewBftMessage::view_change_value_desc(view_change_value)
            );

            return false;
        }
        return true;
    }
}
