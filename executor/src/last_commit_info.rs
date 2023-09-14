use protos::{common::ValidatorSet, consensus::BftProof, ledger::LedgerHeader};
use state::TrieHash;

pub struct LastCommittedInfo {
    pub header: LedgerHeader,
    pub validators: ValidatorSet,
    pub proof: Option<BftProof>,
}

impl Default for LastCommittedInfo {
    fn default() -> Self {
        LastCommittedInfo {
            header: Default::default(),
            validators: Default::default(),
            proof: Default::default(),
        }
    }
}

impl LastCommittedInfo {
    pub fn update(
        &mut self,
        header: &LedgerHeader,
        validators: &ValidatorSet,
        proof: Option<BftProof>,
    ) {
        self.header.clone_from(header);
        self.validators.clone_from(validators);
        self.proof.clone_from(&proof);
    }

    pub fn get_header(&self) -> &LedgerHeader {
        &self.header
    }

    pub fn get_validators(&self) -> &ValidatorSet {
        &self.validators
    }

    pub fn get_state_hash(&self) -> TrieHash {
        let mut hash = TrieHash::default();
        hash.clone_from_slice(self.header.get_state_hash());
        hash
    }

    pub fn get_proof(&self) -> Option<BftProof> {
        self.proof.clone()
    }
}
