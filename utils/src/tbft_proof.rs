use crate::general::{self_chain_hub, self_chain_id};
use msp::signing::eddsa_ed25519::{EddsaEd25519Context, EddsaEd25519PublicKey};
use msp::signing::{Context, PublicKey};
use msp::{bytes_to_hex_str, hex_str_to_bytes};
use protobuf::{Message, RepeatedField};
use protos::common::Signature;
use protos::consensus::*;
use std::collections::HashMap;

pub struct TendermintProof {
    pub proposal_hash: Vec<u8>,
    // Prev height
    pub height: i64,
    pub round: i32,
    pub commits: HashMap<String, Signature>,
}

impl Clone for TendermintProof {
    fn clone(&self) -> Self {
        TendermintProof {
            height: self.height,
            round: self.round,
            proposal_hash: self.proposal_hash.clone(),
            commits: self.commits.clone(),
        }
    }
}

impl ::std::fmt::Debug for TendermintProof {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(
            f,
            "TendermintProof {{ \
             h: {}, r: {}, proposal: {:?}, commits: {:?} \
             }}",
            self.height,
            self.round,
            bytes_to_hex_str(&self.proposal_hash),
            self.commits.keys()
        )
    }
}

impl ::std::fmt::Display for TendermintProof {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(
            f,
            "TendermintProof {{ \
             h: {}, r: {}, proposal: {:?}, commits: {:?} \
             }}",
            self.height,
            self.round,
            bytes_to_hex_str(&self.proposal_hash),
            self.commits.keys()
        )
    }
}

impl TendermintProof {
    pub fn new(
        height: i64,
        round: i32,
        proposal_hash: Vec<u8>,
        commits: HashMap<String, Signature>,
    ) -> TendermintProof {
        TendermintProof {
            height,
            round,
            proposal_hash,
            commits,
        }
    }

    pub fn from_protocal(proto_proof: &TbftProof) -> TendermintProof {
        let mut commits: HashMap<String, Signature> = HashMap::new();

        for sig in proto_proof.get_commits().iter() {
            let sender = Self::pubkey_to_address(sig);
            commits.insert(sender, sig.clone());
        }

        TendermintProof {
            height: proto_proof.get_height(),
            round: proto_proof.get_round(),
            proposal_hash: proto_proof.get_proposal_hash().to_vec(),
            commits,
        }
    }

    pub fn to_protocal(&self) -> TbftProof {
        let mut proto_proof = TbftProof::default();
        proto_proof.set_height(self.height);
        proto_proof.set_round(self.round);
        proto_proof.set_proposal_hash(self.proposal_hash.clone());
        let mut sigs = Vec::new();
        for (sender, sig) in self.commits.iter() {
            sigs.push(sig.clone());
        }
        proto_proof.set_commits(RepeatedField::from(sigs));
        proto_proof
    }

    pub fn from_consensus_proof(proof: &ConsensusProof) -> TendermintProof {
        Self::from_protocal(proof.get_tbft_proof())
    }

    pub fn to_consensus_proof(&self) -> ConsensusProof {
        let mut proof = ConsensusProof::default();
        proof.set_ctype(ConsensusType::TBFT);
        proof.set_tbft_proof(self.to_protocal());
        proof
    }

    pub fn default() -> Self {
        TendermintProof {
            height: i64::MAX,
            round: i32::MAX,
            proposal_hash: Vec::default(),
            commits: HashMap::new(),
        }
    }

    pub fn is_default(&self) -> bool {
        if self.round == i32::MAX {
            return true;
        }
        false
    }
    fn verify_bft_message(bft_sign: &TbftSign) -> bool {
        let sig = bft_sign.get_signature();
        let bft = bft_sign.get_bft();
        let content = bft.write_to_bytes().unwrap();
        let ctx = EddsaEd25519Context::default();
        let pub_key = EddsaEd25519PublicKey::from_bytes(sig.get_public_key());
        if pub_key.is_err() {
            return false;
        }

        let result = ctx.verify(sig.get_sign_data(), content.as_slice(), &pub_key.unwrap());
        if result.is_err() {
            return false;
        }
        result.unwrap()
    }
    fn pubkey_to_address(sig: &Signature) -> String {
        let address =
            EddsaEd25519PublicKey::from_hex(bytes_to_hex_str(sig.get_public_key()).as_str())
                .unwrap()
                .get_address();
        address
    }

    // Check proof commits
    pub fn check(&self, h: i64, authorities: &[String]) -> bool {
        if h == 0 {
            return true;
        }
        if h != self.height {
            return false;
        }
        if 2 * authorities.len() >= 3 * self.commits.len() {
            return false;
        }
        self.commits.iter().all(|(sender, sig)| {
            if authorities.contains(sender) {
                let mut proto_vote = TbftVote::new();
                proto_vote.set_height(h);
                proto_vote.set_round(self.round);

                let step = 4; //Step::Precommit;
                proto_vote.set_step(step);
                proto_vote.set_sender(sender.clone());
                let mut hash = TbftVoteProposalHash::default();
                hash.set_hash(self.proposal_hash.clone());
                proto_vote.set_proposal_hash(hash);

                let mut vote_bft = Tbft::new();
                vote_bft.set_msg_type(TbftMessageType::TPFT_VOTE);
                vote_bft.set_chain_hub(self_chain_hub());
                vote_bft.set_chain_id(self_chain_id());
                vote_bft.set_vote(proto_vote);

                let mut vote_bft_sign = TbftSign::new();
                vote_bft_sign.set_bft(vote_bft);
                vote_bft_sign.set_signature(sig.clone());

                if Self::verify_bft_message(&vote_bft_sign) {
                    return Self::pubkey_to_address(vote_bft_sign.get_signature()) == *sender;
                }
            }
            false
        })
    }
}
