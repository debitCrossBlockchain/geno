use std::collections::HashMap;

use crate::contract::{ContractContext, ContractParameter, SystemContractTrait};
use anyhow::{bail, Ok};
use protos::common::{ContractEvent, ContractResult, ValidatorSet};
use serde::{Deserialize, Serialize};
use utils::parse::ProtocolParser;

const PROPOSAL_LIST_KEY: &str = "validators_proposal_list";
pub const VALIDATORS_KEY: &str = "validators_key";
const ADDRESS_PREFIX: &str = "0x";
const ADDRESS_LENGTH: usize = 42;
const PROPOSAL_NAME_LENGTH: usize = 30;

const PROPOSAL_ACTION_ADD: u64 = 0;
const PROPOSAL_ACTION_DEL: u64 = 1;

const PROPOSAL_STATE_INITIATED: u64 = 1;
const PROPOSAL_STATE_APPROVED: u64 = 2;
const PROPOSAL_STATE_REVOKED: u64 = 3;
const PROPOSAL_STATE_EXPIRED: u64 = 4;

pub struct ValidatorsElectContract {
    pub context: ContractContext,
}

impl SystemContractTrait for ValidatorsElectContract {
    type Context = ContractContext;

    fn dispatch(
        &mut self,
        function: &str,
        params: ContractParameter,
    ) -> anyhow::Result<ContractResult> {
        if function == "proposal" {
            return self.proposal(params);
        } else if function == "vote" {
            return self.vote(params);
        } else if function == "revoke" {
            return self.revoke(params);
        } else if function == "query" {
            return self.query(params);
        } else {
            bail!("unknown function");
        }
    }

    fn init_context(&mut self, context: Self::Context) {
        self.context.clone_from(&context);
    }
    fn contract_address(&self) -> String {
        self.context.base_info.address.clone()
    }
    fn invoker_address(&self) -> String {
        self.context.base_info.invoker.clone()
    }
    fn block_height(&self) -> u64 {
        self.context.base_info.block_height
    }
    fn block_timestamp(&self) -> i64 {
        self.context.base_info.block_timestamp
    }
    fn tx_hash(&self) -> String {
        self.context.base_info.tx_hash.clone()
    }
}

#[derive(Serialize, Deserialize)]
struct ProposalRequest {
    name: String,
    candidate: String,
    action: u64,
    limit: u64,
}

#[derive(Serialize, Deserialize)]
struct VoteRequest {
    name: String,
}

#[derive(Serialize, Deserialize)]
struct RevokeRequest {
    name: String,
}

#[derive(Serialize, Deserialize)]
pub struct QueryParams {
    state: u64,
}

#[derive(Serialize, Deserialize)]
struct Proposal {
    name: String,
    candidate: String,
    action: u64,
    sponsor: String,
    limit: u64,
    voter: Vec<String>,
    state: u64,
}

#[derive(Serialize, Deserialize)]
struct ProposalSet {
    proposals: HashMap<String, u64>,
}

impl ValidatorsElectContract {
    pub fn new(contract_address: String) -> ValidatorsElectContract {
        let mut context = ContractContext::default();
        context.base_info.address.clone_from(&contract_address);
        ValidatorsElectContract { context }
    }

    fn is_hex(address: &str) -> bool {
        for i in address.chars() {
            if !(('0' <= i && i <= '9') || ('a' <= i && i <= 'f')) {
                return false;
            }
        }
        true
    }

    fn check_address(mut address: &str) -> bool {
        let mut new_address = address;
        if address.len() == ADDRESS_LENGTH && &address[0..2] == ADDRESS_PREFIX {
            new_address = &address[ADDRESS_PREFIX.len()..];
            if Self::is_hex(new_address) {
                return true;
            }
        }
        false
    }

    fn check_proposal_name(name: &str) -> bool {
        if name.len() > PROPOSAL_NAME_LENGTH {
            return false;
        }
        for i in name.chars() {
            if !(('0' <= i && i <= '9') || ('a' <= i && i <= 'z') || ('A' <= i && i <= 'Z')) {
                return false;
            }
        }
        true
    }

    fn make_proposal_key(name: &str) -> String {
        let mut s = String::from("proposal-");
        s.push_str(name);
        s
    }

    fn get_quorum_size(size: usize) -> usize {
        // N       1   2   3   4   5   6   7   8   9
        // quorum  0   1   1   2   3   3   4   5   5
        // q +1    1   2   2   3   4   4   5   6   6
        if size == 1 {
            return 0;
        }
        if size == 2 || size == 3 {
            return 1;
        }
        let fault_number = (size - 1) / 3;
        let mut quorum_size = size;
        if size == 3 * fault_number + 1 {
            quorum_size = 2 * fault_number;
        } else if size == 3 * fault_number + 2 {
            quorum_size = 2 * fault_number + 1;
        } else if size == 3 * fault_number + 3 {
            quorum_size = 2 * fault_number + 1;
        }
        return quorum_size;
    }

    fn load_validators(&self) -> anyhow::Result<ValidatorSet> {
        let self_address = self.context.base_info.address.clone();
        let account = self.context.state.get(&self_address)?;
        let mut account = match account {
            Some(a) => a,
            None => {
                anyhow::bail!("validators contract not found");
            }
        };

        let data = account.get_contract_metadata(VALIDATORS_KEY.as_bytes())?;
        let data = match data {
            Some(a) => a,
            None => {
                anyhow::bail!("validators contract no validators data");
            }
        };
        let validators = ProtocolParser::deserialize::<ValidatorSet>(data.as_slice())?;
        Ok(validators)
    }

    fn save_validators(&self, validators: &ValidatorSet) -> anyhow::Result<()> {
        let value_bytes = ProtocolParser::serialize::<ValidatorSet>(validators);
        let self_address = self.context.base_info.address.clone();
        let account = self.context.state.get(&self_address)?;
        let mut account = match account {
            Some(a) => a,
            None => {
                anyhow::bail!("validators contract not found");
            }
        };

        account.upsert_contract_metadata(VALIDATORS_KEY.as_bytes(), &value_bytes);
        self.context.state.upsert(&self_address, account);
        Ok(())
    }

    fn load_proposal_list(&self) -> anyhow::Result<ProposalSet> {
        let self_address = self.context.base_info.address.clone();
        let account = self.context.state.get(&self_address)?;
        let mut account = match account {
            Some(a) => a,
            None => {
                anyhow::bail!("validators contract not found");
            }
        };

        let data = account.get_contract_metadata(PROPOSAL_LIST_KEY.as_bytes())?;
        let data = match data {
            Some(a) => a,
            None => {
                return Ok(ProposalSet {
                    proposals: HashMap::new(),
                })
            }
        };

        let proposal_set: ProposalSet = bincode::deserialize(&data)?;
        Ok(proposal_set)
    }

    fn save_proposal_list(&self, proposal_set: ProposalSet) -> anyhow::Result<()> {
        let value_bytes = bincode::serialize(&proposal_set)?;

        let self_address = self.context.base_info.address.clone();
        let account = self.context.state.get(&self_address)?;
        let mut account = match account {
            Some(a) => a,
            None => {
                anyhow::bail!("validators contract not found");
            }
        };

        account.upsert_contract_metadata(PROPOSAL_LIST_KEY.as_bytes(), value_bytes.as_slice());
        self.context.state.upsert(&self_address, account);
        Ok(())
    }

    fn save_proposal(&self, proposal: Proposal) -> anyhow::Result<()> {
        let value_bytes = bincode::serialize(&proposal)?;

        let self_address = self.context.base_info.address.clone();
        let account = self.context.state.get(&self_address)?;
        let mut account = match account {
            Some(a) => a,
            None => {
                anyhow::bail!("validators contract not found");
            }
        };

        account.upsert_contract_metadata(
            Self::make_proposal_key(proposal.name.as_str()).as_bytes(),
            value_bytes.as_slice(),
        );
        self.context.state.upsert(&self_address, account);
        Ok(())
    }

    fn load_proposal(&self, name: &str) -> anyhow::Result<Option<Proposal>> {
        let self_address = self.context.base_info.address.clone();
        let account = self.context.state.get(&self_address)?;
        let mut account = match account {
            Some(a) => a,
            None => {
                anyhow::bail!("validators contract not found");
            }
        };

        let data = account.get_contract_metadata(Self::make_proposal_key(name).as_bytes())?;
        let data = match data {
            Some(a) => a,
            None => return Ok(None),
        };

        let proposal: Proposal = bincode::deserialize(&data)?;
        Ok(Some(proposal))
    }

    fn validators_contain(validators: &ValidatorSet, address: &str) -> bool {
        for validator in validators.get_validators() {
            if validator.get_address() == address {
                return true;
            }
        }
        false
    }

    fn log_validators(&self, result: &mut ContractResult, validators: &ValidatorSet) {
        let vs: Vec<_> = validators
            .get_validators()
            .iter()
            .map(|x| x.get_address().to_string())
            .collect();

        let mut event = ContractEvent::default();
        event.set_address(self.contract_address());
        event.set_topic(protobuf::RepeatedField::from(vs));

        result.mut_contract_event().push(event);
    }

    fn proposal(&mut self, params: ContractParameter) -> anyhow::Result<ContractResult> {
        let mut result = ContractResult::new();
        let proposal_param: ProposalRequest = serde_json::from_str(params.to_string().as_str())?;

        if !Self::check_address(proposal_param.candidate.as_str())
            || !Self::check_proposal_name(proposal_param.name.as_str())
            || (proposal_param.action != PROPOSAL_ACTION_ADD
                && proposal_param.action != PROPOSAL_ACTION_DEL)
        {
            bail!("proposal contact parameter error");
        }

        // laod validators
        let validators = self.load_validators()?;
        let mut proposal_set = self.load_proposal_list()?;
        let invoker = self.invoker_address();
        if !Self::validators_contain(&validators, &invoker) {
            bail!("invoker not be validators");
        }

        // proposal name must not in proposal name list
        if proposal_set.proposals.contains_key(&proposal_param.name) {
            bail!("same proposal already is exist");
        }

        if proposal_param.action == PROPOSAL_ACTION_ADD {
            if Self::validators_contain(&validators, &proposal_param.candidate) {
                bail!("candidate already is validators");
            }
        } else if proposal_param.action == PROPOSAL_ACTION_DEL {
            if !Self::validators_contain(&validators, &proposal_param.candidate) {
                bail!("candidate not in validators");
            }

            if validators.get_validators().len() == 1 {
                bail!("candidate is validators,only one left");
            }
        }

        // save proposal into metadata
        let proposal = Proposal {
            name: proposal_param.name.clone(),
            candidate: proposal_param.candidate.clone(),
            action: proposal_param.action,
            sponsor: self.invoker_address(),
            limit: self.block_height() + proposal_param.limit,
            voter: Vec::new(),
            state: PROPOSAL_STATE_INITIATED,
        };
        self.save_proposal(proposal)?;

        //add proposal name into state
        proposal_set
            .proposals
            .insert(proposal_param.name.clone(), 0);

        self.save_proposal_list(proposal_set)?;

        Ok(result)
    }

    fn vote(&mut self, params: ContractParameter) -> anyhow::Result<ContractResult> {
        let mut result = ContractResult::new();
        let vote_param: VoteRequest = serde_json::from_str(params.to_string().as_str())?;

        // laod validators
        let mut validators = self.load_validators()?;
        let proposal_set = self.load_proposal_list()?;
        let invoker = self.invoker_address();
        if !Self::validators_contain(&validators, &invoker) {
            bail!("invoker not be validators");
        }

        // proposal name must not in proposal name list
        if !proposal_set.proposals.contains_key(&vote_param.name) {
            bail!("proposal is not exist");
        }

        let proposal = self.load_proposal(&vote_param.name)?;
        let mut proposal = match proposal {
            Some(p) => p,
            None => bail!("proposal is not exist"),
        };

        match proposal.state {
            PROPOSAL_STATE_INITIATED => {
                if proposal.limit <= self.block_height() {
                    proposal.state = PROPOSAL_STATE_EXPIRED;
                    self.save_proposal(proposal)?;
                } else {
                    if !proposal.voter.contains(&self.invoker_address()) {
                        proposal.voter.push(self.invoker_address());
                        if proposal.voter.len()
                            >= Self::get_quorum_size(validators.get_validators().len()) + 1
                        {
                            if proposal.action == PROPOSAL_ACTION_ADD {
                                let mut v = protos::common::Validator::new();
                                v.set_address(proposal.candidate.clone());
                                validators.mut_validators().push(v);
                                proposal.state = PROPOSAL_STATE_APPROVED;
                                self.log_validators(&mut result, &validators);
                            }
                            if proposal.action == PROPOSAL_ACTION_DEL {
                                let vs: Vec<_> = validators
                                    .get_validators()
                                    .iter()
                                    .filter(|&x| x.address != proposal.candidate)
                                    .collect();
                                let vs2: Vec<_> = vs.iter().map(|&x| x.clone()).collect();
                                validators.set_validators(protobuf::RepeatedField::from(vs2));
                                proposal.state = PROPOSAL_STATE_APPROVED;
                                self.log_validators(&mut result, &validators);
                            }
                        }
                        self.save_proposal(proposal)?;
                    } else {
                        bail!("invoker already vote");
                    }
                }
            }
            PROPOSAL_STATE_EXPIRED => {
                bail!("VoteFail EXPIRED");
            }
            PROPOSAL_STATE_REVOKED => {
                bail!("VoteFail REVOKED");
            }
            PROPOSAL_STATE_APPROVED => {
                bail!("VoteFail APPROVED");
            }
            _ => {}
        }

        Ok(result)
    }

    pub fn revoke(&mut self, params: ContractParameter) -> anyhow::Result<ContractResult> {
        let mut result = ContractResult::new();
        let revoke_param: VoteRequest = serde_json::from_str(params.to_string().as_str())?;

        // laod validators
        let validators = self.load_validators()?;
        let proposal_set = self.load_proposal_list()?;
        let invoker = self.invoker_address();
        if !Self::validators_contain(&validators, &invoker) {
            bail!("invoker not be validators");
        }

        // proposal name must not in proposal name list
        if !proposal_set.proposals.contains_key(&revoke_param.name) {
            bail!("proposal is not exist");
        }

        let proposal = self.load_proposal(&revoke_param.name)?;
        let mut proposal = match proposal {
            Some(p) => p,
            None => bail!("proposal is not exist"),
        };

        if proposal.sponsor != self.invoker_address() {
            bail!("not sponsor revoke");
        }

        match proposal.state {
            PROPOSAL_STATE_INITIATED => {
                if proposal.limit <= self.block_height() {
                    proposal.state = PROPOSAL_STATE_EXPIRED;
                } else {
                    proposal.state = PROPOSAL_STATE_REVOKED;
                }
                self.save_proposal(proposal)?;
            }
            PROPOSAL_STATE_EXPIRED => {
                bail!("RevokeFail EXPIRED");
            }
            PROPOSAL_STATE_REVOKED => {
                bail!("RevokeFail REVOKED");
            }
            PROPOSAL_STATE_APPROVED => {
                bail!("RevokeFail APPROVED");
            }
            _ => {}
        }

        Ok(result)
    }

    pub fn query(&mut self, params: ContractParameter) -> anyhow::Result<ContractResult> {
        let mut result = ContractResult::new();
        let query_param: QueryParams = serde_json::from_str(params.to_string().as_str())?;

        let proposal_set = self.load_proposal_list()?;

        let mut arr: Vec<serde_json::Value> = Vec::new();
        for (proposal_name, _) in proposal_set.proposals.iter() {
            // load proposal
            let proposal = self.load_proposal(proposal_name)?;
            if let Some(proposal) = proposal {
                if query_param.state == 0 || proposal.state == query_param.state {
                    let value = serde_json::from_str(&serde_json::to_string(&proposal)?)?;
                    arr.push(value);
                }
            }
        }
        let proposals = serde_json::Value::Array(arr);
        result.set_message(proposals.to_string());
        Ok(result)
    }
}
