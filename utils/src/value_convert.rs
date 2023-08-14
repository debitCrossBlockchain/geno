use protos::consensus::{BftProof, ConsensusProof, ConsensusType, ValidatorSet};
use protos::ledger::{BftValue, BftValueV1000};

#[derive(Default, Clone)]
pub struct ValueConvert;

impl ValueConvert {
    pub fn to_bft_value(bft_value_v1000: &BftValueV1000) -> BftValue {
        let mut bft_value = BftValue::new();

        bft_value.set_ledger_seq(bft_value_v1000.get_ledger_seq());
        bft_value.set_ledger_timestamp(bft_value_v1000.get_ledger_timestamp());
        bft_value.set_previous_ledger_hash(Vec::from(bft_value_v1000.get_previous_ledger_hash()));

        if bft_value_v1000.has_tx_set() {
            bft_value.set_tx_set(bft_value_v1000.get_tx_set().clone());
        }
        if bft_value_v1000.has_previous_proof() {
            let mut proof = ConsensusProof::new();
            proof.set_ctype(ConsensusType::PBFT);
            proof.set_pbft_proof(bft_value_v1000.get_previous_proof().clone());
            bft_value.set_previous_proof(proof);
        }

        if bft_value_v1000.has_ledger_upgrade() {
            bft_value.set_ledger_upgrade(bft_value_v1000.get_ledger_upgrade().clone());
        }
        if bft_value_v1000.has_validation() {
            bft_value.set_validation(bft_value_v1000.get_validation().clone());
        }

        bft_value
    }

    pub fn to_bft_value_v1000(bft_value: &BftValue) -> BftValueV1000 {
        let mut bft_value_v1000 = BftValueV1000::new();

        bft_value_v1000.set_ledger_seq(bft_value.get_ledger_seq());
        bft_value_v1000.set_ledger_timestamp(bft_value.get_ledger_timestamp());
        bft_value_v1000.set_previous_ledger_hash(Vec::from(bft_value.get_previous_ledger_hash()));

        if bft_value.has_tx_set() {
            bft_value_v1000.set_tx_set(bft_value.get_tx_set().clone());
        }
        if bft_value.has_previous_proof() && bft_value.get_previous_proof().has_pbft_proof() {
            bft_value_v1000
                .set_previous_proof(bft_value.get_previous_proof().get_pbft_proof().clone());
        }

        if bft_value.has_ledger_upgrade() {
            bft_value_v1000.set_ledger_upgrade(bft_value.get_ledger_upgrade().clone());
        }
        if bft_value.has_validation() {
            bft_value_v1000.set_validation(bft_value.get_validation().clone());
        }

        bft_value_v1000
    }
}
