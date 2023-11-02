use std::collections::HashMap;

use crate::{
    contract::{ContractContext, ContractParameter, SystemContractTrait},
    system_address::get_system_address,
};
use anyhow::{bail, Ok};
use protos::common::{ContractEvent, ContractResult, ValidatorSet};
use serde::{Deserialize, Serialize};
use utils::parse::ProtocolParser;

const PROPOSAL_RECORDES_KEY: &str = "proposalRecordsKey";
pub const RECORDK_KEY: &str = "voteRecords_";
pub const VALIDATORS_KEY: &str = "validators_key";
const NONCE_KEY: &str = "nonce";
const ADDRESS_PREFIX: &str = "0x";
const ADDRESS_LENGTH: usize = 42;
const PROPOSAL_NAME_LENGTH: usize = 30;

pub struct FeesElect {
    pub context: ContractContext,
}

impl SystemContractTrait for FeesElect {
    type Context = ContractContext;

    fn dispatch(
        &mut self,
        function: &str,
        params: ContractParameter,
    ) -> anyhow::Result<ContractResult> {
        if function == "proposal" {
            return self.proposal(params);
        }
        let result = ContractResult::new();
        Ok(result)
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
    proposal_id: String,
    limit: u64,
    fee_type: u64,
    price: u64,
}

#[derive(Serialize, Deserialize)]
struct VoteRequest {
    proposal_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct QueryParams {
    state: u64,
}

#[derive(Serialize, Deserialize)]
struct Proposal {
    sponsor: String,
    proposal_id: String,
    fee_type: u64,
    price: u64,
    voter: Vec<String>,
    limit: u64,
    state: u64,
}

#[derive(Serialize, Deserialize)]
struct ProposalSet {
    proposals: HashMap<String, u64>,
}

impl FeesElect {
    pub fn new(contract_address: String) -> FeesElect {
        let mut context = ContractContext::default();
        context.base_info.address.clone_from(&contract_address);
        FeesElect { context }
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
        let mut s = String::from("voteRecords_");
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
        let validators_address = get_system_address(0).expect("get validators elect address");
        let account = self.context.state.get(&validators_address)?;
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

    fn is_validator(&self, address: &str) -> anyhow::Result<bool> {
        let validators = self.load_validators()?;
        return Ok(Self::validators_contain(&validators, address));
    }

    fn load_proposal_list(&self) -> anyhow::Result<ProposalSet> {
        let self_address = self.context.base_info.address.clone();
        let account = self.context.state.get(&self_address)?;
        let mut account = match account {
            Some(a) => a,
            None => {
                anyhow::bail!("Fees contract not found");
            }
        };

        let data = account.get_contract_metadata(PROPOSAL_RECORDES_KEY.as_bytes())?;
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
                anyhow::bail!("Fees contract not found");
            }
        };

        account.upsert_contract_metadata(PROPOSAL_RECORDES_KEY.as_bytes(), value_bytes.as_slice());
        self.context.state.upsert(&self_address, account);
        Ok(())
    }

    fn storage(&self, key: &[u8], value_bytes: Vec<u8>) -> anyhow::Result<()> {
        let self_address = self.context.base_info.address.clone();
        let account = self.context.state.get(&self_address)?;
        let mut account = match account {
            Some(a) => a,
            None => {
                anyhow::bail!("Fees contract not found");
            }
        };

        account.upsert_contract_metadata(key, value_bytes.as_slice());
        self.context.state.upsert(&self_address, account);
        Ok(())
    }

    fn delete_proposal(&self, proposal: Proposal) -> anyhow::Result<()> {
        let self_address = self.context.base_info.address.clone();
        let account = self.context.state.get(&self_address)?;
        let mut account = match account {
            Some(a) => a,
            None => {
                anyhow::bail!("Fees contract not found");
            }
        };
        account.delete_metadata(
            Self::make_proposal_key(proposal.proposal_id.as_str())
                .as_bytes()
                .to_vec(),
        );
        self.context.state.upsert(&self_address, account);
        Ok(())
    }

    fn save_proposal(&self, proposal: Proposal) -> anyhow::Result<()> {
        let value_bytes = bincode::serialize(&proposal)?;
        let proposal_id = Self::make_proposal_key(proposal.proposal_id.as_str());
        self.storage(proposal_id.as_bytes(), value_bytes)
    }

    fn load_proposal(&self, name: &str) -> anyhow::Result<Option<Proposal>> {
        let self_address = self.context.base_info.address.clone();
        let account = self.context.state.get(&self_address)?;
        let mut account = match account {
            Some(a) => a,
            None => {
                anyhow::bail!("Fees contract not found");
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

    fn proposal(&mut self, params: ContractParameter) -> anyhow::Result<ContractResult> {
        let mut result = ContractResult::new();
        let proposal_param: ProposalRequest = serde_json::from_str(params.to_string().as_str())?;

        if !Self::check_address(self.invoker_address().as_str())
            || !Self::check_proposal_name(proposal_param.proposal_id.as_str())
        {
            bail!("proposal contact parameter error");
        }

        // laod validators
        let mut proposal_set = self.load_proposal_list()?;
        let invoker = self.invoker_address();
        if !self.is_validator(&invoker)? {
            bail!("invoker not be validators");
        }

        // proposal name must not in proposal name list
        if proposal_set
            .proposals
            .contains_key(&proposal_param.proposal_id)
        {
            bail!("same proposal already is exist");
        }

        // save proposal into metadata
        let proposal = Proposal {
            sponsor: self.invoker_address(),
            limit: self.block_height() + proposal_param.limit,
            voter: Vec::new(),
            fee_type: proposal_param.fee_type,
            price: proposal_param.price,
            proposal_id: proposal_param.proposal_id.clone(),
            state: 0,
        };
        self.save_proposal(proposal)?;

        //add proposal name into state
        proposal_set
            .proposals
            .insert(proposal_param.proposal_id.clone(), 0);

        self.save_proposal_list(proposal_set)?;

        Ok(result)
    }

    fn vote(&mut self, params: ContractParameter) -> anyhow::Result<ContractResult> {
        let mut result = ContractResult::new();
        let vote_param: VoteRequest = serde_json::from_str(params.to_string().as_str())?;

        // laod validators
        let mut proposal_set = self.load_proposal_list()?;
        let invoker = self.invoker_address();
        if !self.is_validator(&invoker)? {
            bail!("invoker not be validators");
        }

        let key = Self::make_proposal_key(&vote_param.proposal_id);

        // proposal name must not in proposal name list
        if !proposal_set.proposals.contains_key(&key) {
            bail!("proposal is not exist");
        }

        let mut proposal = self.load_proposal(&key)?;
        let mut proposal = match proposal {
            Some(p) => p,
            None => bail!("proposal is not exist"),
        };

        if proposal.limit <= self.block_height() {
            proposal_set.proposals.remove(&key);
            self.save_proposal_list(proposal_set)?;
            self.delete_proposal(proposal)?;
            bail!("VoteFail EXPIRED");
        }

        if !proposal.voter.contains(&self.invoker_address()) {
            proposal.voter.push(self.invoker_address());
            let validators = self.load_validators()?;
            if proposal.voter.len() >= Self::get_quorum_size(validators.get_validators().len()) + 1
            {
                proposal.state = 1;
                self.save_proposal(proposal)?;
                //save config fee todo
            } else {
                bail!("invoker already vote");
            }
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
