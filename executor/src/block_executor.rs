use crate::block_result::BlockResult;
use crate::block_verify::Verify;
use crate::LAST_COMMITTED_BLOCK_INFO_REF;
use anyhow::bail;
use ledger_store::LedgerStorage;
use merkletree::Tree;
use msp::bytes_to_hex_str;
use protos::{
    common::{TransactionResult, Validator, ValidatorSet},
    consensus::BftProof,
    ledger::*,
};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use state::{cache_state::StateMapActionType, AccountFrame, CacheState, TrieHash, TrieWriter};
use state_store::StateStorage;
use std::collections::HashMap;
use storage_db::{MemWriteBatch, WriteBatchTrait, STORAGE_INSTANCE_REF};
use syscontract::system_address::is_system_contract;
use tracing::info;
use types::error::BlockExecutionError;
use types::transaction::SignedTransaction;
use utils::{
    general::{genesis_block_config, hash_crypto_byte, hash_zero, self_chain_hub, self_chain_id},
    parse::ProtocolParser,
};

use vm::{EvmExecutor, PostState};
pub struct BlockExecutor {}

impl BlockExecutor {
    pub fn execute_block(
        block: &Ledger,
    ) -> std::result::Result<(Vec<SignedTransaction>, BlockResult), BlockExecutionError> {
        let header = block.get_header();

        // initialize state by last block state root
        let root_hash = TrieHash::default();
        let state = CacheState::new(root_hash);

        // initialize contract vm
        let mut vm = match EvmExecutor::new(header, state.clone()) {
            Ok(vm) => vm,
            Err(e) => {
                return Err(BlockExecutionError::VmError {
                    error: format!("vm init error {e:?}"),
                });
            }
        };

        let mut post_state = PostState::new();

        // execute block
        let mut tx_array = Vec::with_capacity(block.get_transaction_signs().len());
        for (index, tx) in block.get_transaction_signs().iter().enumerate() {
            let tx_raw = match SignedTransaction::try_from(tx.clone()) {
                Ok(tx_raw) => tx_raw,
                Err(e) => {
                    let error = BlockExecutionError::TransactionParamError {
                        error: e.to_string(),
                    };
                    continue;
                }
            };
            if is_system_contract(&tx_raw.sender().to_string()) {
            } else {
                if let Err(e) = vm.execute(index, &tx_raw, &mut post_state) {
                    let error = BlockExecutionError::VmError {
                        error: format!("vm execute error {e:?}"),
                    };
                    continue;
                }
            }

            tx_array.push(tx_raw);
        }
        if let Err(e) = post_state.convert_to_geno_state(header.get_height(), state.clone()) {
            return Err(BlockExecutionError::StateConvertError {
                error: format!("{e:?}"),
            });
        }
        state.commit();
        let tx_result_set = post_state.convert_to_geno_txresult(header.get_height());

        let new_validator_set = {
            LAST_COMMITTED_BLOCK_INFO_REF
                .read()
                .get_validators()
                .clone()
        };
        let result = BlockResult {
            state,
            tx_result_set,
            validator_set: new_validator_set,
        };

        Ok((tx_array, result))
    }

    pub fn commit_block(
        block: &mut Ledger,
        txs: Vec<SignedTransaction>,
        result: &BlockResult,
    ) -> anyhow::Result<()> {
        let mut header = block.get_header().clone();

        let last_state_root_hash = if header.get_height() == utils::general::GENESIS_HEIGHT {
            None
        } else {
            Some(LAST_COMMITTED_BLOCK_INFO_REF.read().get_state_hash())
        };

        // state commit and storage
        let mut state_batch = MemWriteBatch::new();
        let mut state_datas = HashMap::new();
        let state_changes = result.state.get_commit_changes();
        for (address, mut value) in state_changes {
            value.data.commit_metadata_trie(&mut state_batch)?;
            match value.action {
                StateMapActionType::UPSERT => {
                    state_datas.insert(address.as_bytes().to_vec(), Some(value.data.serialize()));

                    // store code hash : address
                    if value.data.has_contract() {
                        StateStorage::store_codehash_address_map(
                            &value.data.contract_code_hash(),
                            &address,
                            &mut state_batch,
                        );
                    }
                }
                StateMapActionType::DELETE => {
                    state_datas.insert(address.as_bytes().to_vec(), None);
                }
                _ => {}
            }
        }

        let state_db = STORAGE_INSTANCE_REF.account_db();
        let state_root_hash = TrieWriter::commit(
            state_db,
            last_state_root_hash,
            &state_datas,
            &mut state_batch,
        )?;
        let proof = if let Some(proof_data) = Self::get_proof(block) {
            let proof = ProtocolParser::deserialize::<BftProof>(proof_data)?;
            StateStorage::store_last_proof(&mut state_batch, &proof);
            Some(proof)
        } else {
            None
        };
        StateStorage::store_validators(&mut state_batch, &result.validator_set);
        StateStorage::commit(state_batch)?;

        // set state hash
        header.set_state_hash(state_root_hash.to_vec());

        // caculate txs hash
        let mut txs_leafs: Vec<Vec<u8>> = Vec::new();
        let mut receips_leafs: Vec<Vec<u8>> = Vec::new();
        let mut txs_store = HashMap::with_capacity(block.get_transaction_signs().len());
        for (i, t) in txs.iter().enumerate() {
            let mut tx_store = TransactionSignStore::default();
            let tx_hash = t.hash().to_vec();

            tx_store.set_transaction_sign(block.get_transaction_signs().get(i).unwrap().clone());
            tx_store.set_transaction_result(result.tx_result_set.get(i).unwrap().clone());
            txs_store.insert(tx_hash.clone(), tx_store);
            txs_leafs.push(tx_hash.clone());

            let receips_hash = hash_crypto_byte(&ProtocolParser::serialize::<TransactionResult>(
                result.tx_result_set.get(i).unwrap(),
            ));
            receips_leafs.push(receips_hash);
        }
        if txs_leafs.len() > 0 {
            let mut txs_tree = Tree::new();
            txs_tree.build(txs_leafs.clone());
            header.set_transactions_hash(txs_tree.root());
        }

        // caculate receips hash
        if receips_leafs.len() > 0 {
            let mut receips_tree = Tree::new();
            receips_tree.build(receips_leafs.clone());
            header.set_receips_hash(receips_tree.root());
        }

        // caculate fee hash

        // caculate validators hash
        let validator_hash = hash_crypto_byte(&ProtocolParser::serialize::<ValidatorSet>(
            &result.validator_set,
        ));
        header.set_validators_hash(validator_hash);

        // set consensus value hash
        let consensus_hash = Self::caculate_consensus_value_hash(block);
        Self::set_consensus_value_hash(&mut header, consensus_hash.clone());

        // set ledger hash
        header.set_hash(hash_crypto_byte(
            &ProtocolParser::serialize::<LedgerHeader>(&header),
        ));

        info!(
            "commit_block height({}) hash({}) consensus_value_hash({})",
            header.get_height(),
            bytes_to_hex_str(header.get_hash()),
            bytes_to_hex_str(&consensus_hash),
        );

        let mut ledger_batch = MemWriteBatch::new();
        LedgerStorage::store_ledger(&mut ledger_batch, &header, &txs_store);
        LedgerStorage::commit(ledger_batch)?;

        LAST_COMMITTED_BLOCK_INFO_REF
            .write()
            .update(&header, &result.validator_set, proof);
        block.set_header(header);

        Ok(())
    }

    pub fn create_genesis_block() -> (Ledger, BlockResult) {
        let state = CacheState::new(TrieHash::default());

        //create the account of genesis from config
        let genesis_block = genesis_block_config();
        let genesis_account =
            AccountFrame::new(genesis_block.genesis_account.clone(), 100000000000000000);

        state.upsert(&genesis_block.genesis_account, genesis_account);

        //create accounts of validators from config
        let mut validator_set = ValidatorSet::new();
        for address in genesis_block.validators.iter() {
            let account = AccountFrame::new(address.clone(), 0);

            state.upsert(address, account);

            let mut validator = Validator::new();
            validator.set_address(address.clone());
            validator.set_pledge_amount(0);
            validator_set.mut_validators().push(validator);
        }
        state.commit();

        // set bolck header
        let header = Self::initialize_new_header(
            utils::general::GENESIS_HEIGHT,
            hash_zero(),
            utils::general::GENESIS_TIMESTAMP_USECS,
            utils::general::LEDGER_VERSION,
            0,
            0,
            genesis_block.genesis_account.clone(),
        );

        let mut block = Ledger::default();
        block.set_header(header);

        let result = BlockResult {
            state,
            tx_result_set: Vec::new(),
            validator_set,
        };

        (block, result)
    }

    pub fn commit_verify_block(
        block: &Ledger,
        txs: Vec<SignedTransaction>,
        result: &BlockResult,
    ) -> anyhow::Result<()> {
        let mut header = block.get_header().clone();

        let last_state_root_hash = if header.get_height() == utils::general::GENESIS_HEIGHT {
            None
        } else {
            Some(LAST_COMMITTED_BLOCK_INFO_REF.read().get_state_hash())
        };

        // state commit and storage
        let mut state_batch = MemWriteBatch::new();
        let mut state_datas = HashMap::new();
        let state_changes = result.state.get_commit_changes();
        for (address, mut value) in state_changes {
            value.data.commit_metadata_trie(&mut state_batch)?;
            match value.action {
                StateMapActionType::UPSERT => {
                    state_datas.insert(address.as_bytes().to_vec(), Some(value.data.serialize()));

                    // store code hash : address
                    if value.data.has_contract() {
                        StateStorage::store_codehash_address_map(
                            &value.data.contract_code_hash(),
                            &address,
                            &mut state_batch,
                        );
                    }
                }
                StateMapActionType::DELETE => {
                    state_datas.insert(address.as_bytes().to_vec(), None);
                }
                _ => {}
            }
        }

        let state_db = STORAGE_INSTANCE_REF.account_db();
        let state_root_hash = TrieWriter::commit(
            state_db,
            last_state_root_hash,
            &state_datas,
            &mut state_batch,
        )?;

        StateStorage::store_validators(&mut state_batch, &result.validator_set);
        StateStorage::commit(state_batch)?;

        // verify state hash
        match header.verify_state_hash(&state_root_hash) {
            Ok(v) if v == true => (),
            _ => bail!("verify state hash error"),
        };

        // caculate txs hash
        let mut txs_leafs: Vec<Vec<u8>> = Vec::new();
        let mut receips_leafs: Vec<Vec<u8>> = Vec::new();
        let mut txs_store = HashMap::with_capacity(block.get_transaction_signs().len());
        for (i, t) in txs.iter().enumerate() {
            let mut tx_store = TransactionSignStore::default();
            let tx_hash = t.hash().to_vec();

            tx_store.set_transaction_sign(block.get_transaction_signs().get(i).unwrap().clone());
            tx_store.set_transaction_result(result.tx_result_set.get(i).unwrap().clone());
            txs_store.insert(tx_hash.clone(), tx_store);
            txs_leafs.push(tx_hash.clone());
            let receips_hash = hash_crypto_byte(&ProtocolParser::serialize::<TransactionResult>(
                result.tx_result_set.get(i).unwrap(),
            ));
            receips_leafs.push(receips_hash);
        }

        // caculate receips hash
        if txs_leafs.len() > 0 {
            let mut txs_tree = Tree::new();
            txs_tree.build(txs_leafs.clone());
            header.set_transactions_hash(txs_tree.root());
        }

        // caculate receips hash
        if receips_leafs.len() > 0 {
            let mut receips_tree = Tree::new();
            receips_tree.build(receips_leafs.clone());
            header.set_receips_hash(receips_tree.root());
        }

        // caculate fee hash

        // verify validators hash
        let validator_hash = hash_crypto_byte(&ProtocolParser::serialize::<ValidatorSet>(
            &result.validator_set,
        ));
        match header.verify_validators_hash(&validator_hash) {
            Ok(v) if v == true => (),
            _ => bail!("verify validators hash error"),
        };

        // verify block hash  ???
        // let block_hash = hash_crypto_byte(&ProtocolParser::serialize::<LedgerHeader>(&header));
        // match header.verify_block_hash(&block_hash){
        //     Ok(v) if v == true => (),
        //     _ => bail!("verify block hash error"),
        // };

        let mut ledger_batch = MemWriteBatch::new();
        LedgerStorage::store_ledger(&mut ledger_batch, &header, &txs_store);
        LedgerStorage::commit(ledger_batch)?;

        Ok(())
    }

    pub fn verify_block(&self, block: &Ledger) -> anyhow::Result<Vec<bool>> {
        //verify header (todo)
        let header = block.get_header();

        if let Ok(Some(h)) = LedgerStorage::load_max_block_height() {
            if h + 1 != header.get_height() {
                bail!("verify block error!")
            }
        } else {
            bail!("verify block error!!")
        };
        if let Ok(Some(pre_header)) =
            LedgerStorage::load_ledger_header_by_seq(header.get_height() - 1)
        {
            match header.verify_pre_hash(pre_header.get_previous_hash()) {
                Ok(v) if v == true => (),
                _ => bail!("verify previous hash error!"),
            };
        } else {
            bail!("verify previous hash error!!")
        }

        //verify transaction
        let ret: Vec<bool> = block
            .get_transaction_signs()
            .par_iter()
            .map(|tx| match tx.verify_tx() {
                Ok(v) => v,
                Err(_) => false,
            })
            .collect();

        if ret.iter().any(|&r| r == false) {
            bail!("verify block error")
        } else {
            Ok(ret)
        }
    }

    pub fn execute_verify_block(&self, block: Ledger) -> anyhow::Result<()> {
        match self.verify_block(&block) {
            Ok(_) => (),
            Err(e) => bail!(e),
        }

        let (a, b) = match BlockExecutor::execute_block(&block) {
            Ok((a, b)) => (a, b),
            Err(e) => bail!(e),
        };

        BlockExecutor::commit_verify_block(&block, a, &b)
    }

    pub fn block_initialize() -> anyhow::Result<()> {
        let (header, validators, proof) = if let Some(height) =
            LedgerStorage::load_max_block_height()?
        {
            let header = LedgerStorage::load_ledger_header_by_seq(height)?;
            if let Some(header) = header {
                let result = StateStorage::load_validators(&hex::encode(header.get_state_hash()))?;
                if let Some(validators) = result {
                    let proof = StateStorage::load_last_proof()?;
                    (header, validators, proof)
                } else {
                    panic!("block initialize load validators failed:{}", height);
                }
            } else {
                panic!("block initialize load block header failed:{}", height);
            }
        } else {
            let (mut block, block_result) = Self::create_genesis_block();
            if let Err(e) = Self::commit_block(&mut block, Vec::new(), &block_result) {
                panic!("block initialize genesis failed:{}", e);
            }
            (
                block.get_header().clone(),
                block_result.validator_set.clone(),
                None,
            )
        };

        LAST_COMMITTED_BLOCK_INFO_REF
            .write()
            .update(&header, &validators, proof);

        Ok(())
    }

    pub fn initialize_new_header(
        height: u64,
        previous_hash: Vec<u8>,
        timestamp: i64,
        version: u64,
        tx_count: u64,
        total_tx_count: u64,
        proposer: String,
    ) -> LedgerHeader {
        let mut header = LedgerHeader::default();
        header.set_height(height);
        header.set_timestamp(timestamp);
        header.set_previous_hash(previous_hash);
        header.set_version(version);
        header.set_tx_count(tx_count);
        header.set_total_tx_count(total_tx_count);
        header.set_proposer(proposer);
        header.set_chain_id(self_chain_id());
        header.set_hub_id(self_chain_hub());
        header
    }

    pub fn initialize_new_block(
        height: u64,
        previous_hash: Vec<u8>,
        timestamp: i64,
        version: u64,
        tx_count: u64,
        total_tx_count: u64,
        proposer: String,
        previous_proof: Option<Vec<u8>>,
        tx_hash_list: Option<Vec<u8>>,
    ) -> Ledger {
        let mut ledger = Ledger::default();

        let header = Self::initialize_new_header(
            height,
            previous_hash,
            timestamp,
            version,
            tx_count,
            total_tx_count,
            proposer,
        );

        ledger.set_header(header);
        let mut extended_data = ExtendedData::default();
        if let Some(previous_proof) = previous_proof {
            Self::set_previous_proof(&mut ledger, previous_proof);
        }
        if let Some(tx_hash_list) = tx_hash_list {
            Self::set_tx_hash_list(&mut ledger, tx_hash_list);
        }

        ledger
    }

    pub fn get_consensus_value_hash(header: &LedgerHeader) -> Option<&Vec<u8>> {
        header
            .get_extended_data()
            .get_extra_data()
            .get(utils::general::BFT_CONSENSUS_VALUE_HASH)
    }

    pub fn get_previous_proof(block: &Ledger) -> Option<&Vec<u8>> {
        block
            .get_extended_data()
            .get_extra_data()
            .get(utils::general::BFT_PREVIOUS_PROOF)
    }

    pub fn get_tx_hash_list(block: &Ledger) -> Option<&Vec<u8>> {
        block
            .get_extended_data()
            .get_extra_data()
            .get(utils::general::BFT_TX_HASH_LIST)
    }

    pub fn get_proof(block: &Ledger) -> Option<&Vec<u8>> {
        block
            .get_extended_data()
            .get_extra_data()
            .get(utils::general::BFT_CURRENT_PROOF)
    }

    pub fn set_consensus_value_hash(header: &mut LedgerHeader, consensus_hash: Vec<u8>) {
        // insert consensus value hash
        header.mut_extended_data().mut_extra_data().insert(
            utils::general::BFT_CONSENSUS_VALUE_HASH.to_string(),
            consensus_hash,
        );
    }

    pub fn set_current_proof(block: &mut Ledger, proof: Vec<u8>) {
        block
            .mut_extended_data()
            .mut_extra_data()
            .insert(utils::general::BFT_CURRENT_PROOF.to_string(), proof);
    }

    pub fn set_previous_proof(block: &mut Ledger, proof: Vec<u8>) {
        block
            .mut_extended_data()
            .mut_extra_data()
            .insert(utils::general::BFT_PREVIOUS_PROOF.to_string(), proof);
    }

    pub fn set_tx_hash_list(block: &mut Ledger, proof: Vec<u8>) {
        block
            .mut_extended_data()
            .mut_extra_data()
            .insert(utils::general::BFT_TX_HASH_LIST.to_string(), proof);
    }

    pub fn caculate_consensus_value_hash(block: &Ledger) -> Vec<u8> {
        let mut ledger = Ledger::default();
        let header = Self::initialize_new_header(
            block.get_header().get_height(),
            block.get_header().get_previous_hash().to_vec(),
            block.get_header().get_timestamp(),
            block.get_header().get_version(),
            block.get_header().get_tx_count(),
            block.get_header().get_total_tx_count(),
            block.get_header().get_proposer().to_string(),
        );

        ledger.set_header(header);
        if let Some(previous_proof) = block
            .get_extended_data()
            .get_extra_data()
            .get(utils::general::BFT_PREVIOUS_PROOF)
        {
            Self::set_previous_proof(&mut ledger, previous_proof.clone());
        }

        if let Some(tx_hash_list) = block
            .get_extended_data()
            .get_extra_data()
            .get(utils::general::BFT_TX_HASH_LIST)
        {
            Self::set_tx_hash_list(&mut ledger, tx_hash_list.clone());
        }

        // caculate consensus value hash
        let consensus_hash = hash_crypto_byte(&ProtocolParser::serialize::<Ledger>(&ledger));
        consensus_hash
    }

    pub fn call_transaction(tx: &TransactionSign) -> std::result::Result<(), ()> {
        let header = if let Ok(Some(h)) = LedgerStorage::load_max_block_height() {
            if let Ok(Some(header)) = LedgerStorage::load_ledger_header_by_seq(h) {
                header
            } else {
                return Ok(());
            }
        } else {
            return Ok(());
        };

        // initialize state by last block state root
        let root_hash = TrieHash::default();
        let state = CacheState::new(root_hash);

        // initialize contract vm
        let mut vm = match EvmExecutor::new(&header, state.clone()) {
            Ok(vm) => vm,
            Err(e) => {
                return Err(());
            }
        };

        // execute tx
        let tx_raw = match SignedTransaction::try_from(tx.clone()) {
            Ok(tx_raw) => tx_raw,
            Err(e) => return Err(()),
        };
        let ret = match vm.call(&tx_raw) {
            Ok(v) => v,
            Err(e) => return Err(()),
        };

        Ok(())
    }
}
