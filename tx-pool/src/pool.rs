//! pool is used to track transactions which have been submitted but not yet
//! agreed upon.
use crate::types::{CommittedTransaction,Status, StatusCode};
use crate::{
    index::PriorityIndex,
    store::Store,
    transaction::{PoolTransaction, TxState},
};
use network::PeerNetwork;
use protobuf::{Message, RepeatedField};
use protos::common::{ProtocolsMessage, ProtocolsMessageType};
use std::{cmp::max, collections::HashMap, time::Duration};
use utils::TransactionSign;

use types::TransactionSignRaw;
use utils::timing::duration_since_epoch;
pub struct Pool {
    // Stores the metadata of all transactions in pool (of all states).
    transactions: Store,

    seq_cache: HashMap<String, u64>,

    pub transaction_timeout: Duration,

    // for broadcast transactions
    pub broadcast_max_batch_size: usize,
    pub broadcast_cache: Vec<TransactionSignRaw>,
    pub network: Option<PeerNetwork>,
}

impl Pool {
    pub fn new(config: &configure::TxPoolConfig, network: Option<PeerNetwork>) -> Self {
        Pool {
            transactions: Store::new(&config),
            seq_cache: HashMap::with_capacity(config.capacity),
            transaction_timeout: Duration::from_secs(config.system_transaction_timeout_secs),
            broadcast_cache: Vec::new(),
            broadcast_max_batch_size: config.broadcast_max_batch_size,
            network,
        }
    }

    pub fn reinit(&mut self, config: &configure::TxPoolConfig, network: PeerNetwork) {
        self.transactions = Store::new(&config);
        self.seq_cache = HashMap::with_capacity(config.capacity);
        self.broadcast_max_batch_size = config.broadcast_max_batch_size;
        self.transaction_timeout = Duration::from_secs(config.system_transaction_timeout_secs);
        self.network = Some(network);
    }

    pub fn get_by_hash(&self, hash: &[u8]) -> Option<TransactionSignRaw> {
        return self.transactions.get_by_hash(hash);
    }

    pub fn get_bench_by_hash(
        &self,
        hash_list: &[Vec<u8>],
    ) -> (Vec<TransactionSignRaw>, Vec<Vec<u8>>) {
        let mut lack_txs = Vec::new();
        let mut txs = Vec::new();
        for hash in hash_list.iter() {
            if let Some(tx) = self.transactions.get_by_hash(hash) {
                txs.push(tx);
            } else {
                lack_txs.push(hash.clone());
            }
        }
        (txs, lack_txs)
    }

    /// This function will be called once the transaction has been stored.
    pub(crate) fn remove_transaction(&mut self, sender: &str, seq: u64, is_rejected: bool) {
        let current_seq = self
            .seq_cache
            .remove(&sender.to_string())
            .unwrap_or_default();

        // update current cached sequence number for account
        let new_seq_number = max(current_seq, seq);
        self.seq_cache.insert(sender.to_string(), new_seq_number);

        let new_seq_number = seq;
        self.transactions
            .commit_transaction(sender, new_seq_number + 1);
    }

    /// Used to add a transaction to the Mempool.
    /// Performs basic validation: checks account's sequence number.
    // pub(crate) fn add_txn(
    pub fn add_txn(
        &mut self,
        txn: TransactionSignRaw,
        gas_amount: u64,
        ranking_score: u128,
        db_seq: u64,
        tx_state: TxState,
    ) -> Status {
        ///todo log transaction
        let cached_value = self.seq_cache.get(txn.sender());
        let sequence_number = cached_value.map_or(db_seq, |value| max(*value, db_seq));
        self.seq_cache
            .insert(txn.sender().to_string(), sequence_number);

        // don't accept old transactions (e.g. seq is less than account's current seq_number)
        if txn.nonce() <= db_seq {
            return Status::new(StatusCode::InvalidSeqNumber).with_message(format!(
                "transaction sequence number is {}, account sequence number is  {}",
                txn.nonce(),
                db_seq
            ));
        }

        let expiration_time = duration_since_epoch() + self.transaction_timeout;

        let txn_info = PoolTransaction::new(txn.clone(), expiration_time, tx_state, db_seq);

        let status = self.transactions.insert(txn_info);

        if status.code == StatusCode::Accepted
            && (txn.source_type == protos::ledger::TransactionSign_SourceType::JSONRPC
                || txn.source_type == protos::ledger::TransactionSign_SourceType::WEBSOCKET)
        {
            self.broadcast_cache.push(txn);
            if self.broadcast_cache.len() >= self.broadcast_max_batch_size {
                self.broadcast_transaction();
            }
        }
        status
    }

    // broadcast transaction
    pub(crate) fn broadcast_transaction(&mut self) {
        //broadcast msg
        let len = self.broadcast_cache.len();
        if len > 0 {
            let mut broadcast = protos::ledger::TransactionSignBrodcast::default();

            if self.broadcast_cache.len() <= self.broadcast_max_batch_size {
                let vec = self.broadcast_cache.drain(..).collect::<Vec<_>>();
                let mut vec_signs: Vec<TransactionSign> = Vec::new();
                for it in vec {
                    vec_signs.push(it.convert_into());
                }
                broadcast.set_transactions(RepeatedField::from(vec_signs));
            } else {
                let vec = self
                    .broadcast_cache
                    .drain(0..self.broadcast_max_batch_size)
                    .collect::<Vec<_>>();
                let mut vec_signs: Vec<TransactionSign> = Vec::new();
                for it in vec {
                    vec_signs.push(it.convert_into());
                }
                broadcast.set_transactions(RepeatedField::from(vec_signs));
            }

            let mut message = ProtocolsMessage::new();
            message.set_msg_type(ProtocolsMessageType::TRANSACTION);
            message.set_data(broadcast.write_to_bytes().unwrap());
            message.set_timestamp(chrono::Local::now().timestamp_millis());
            if let Some(ref network) = self.network {
                network.broadcast_msg(message);
            }

            let sended_txs = Self::classify(broadcast.get_transactions());
            self.transactions.flag_send(&sended_txs);
        }
    }

    fn classify(arr: &[TransactionSign]) -> HashMap<String, Vec<u64>> {
        let mut map = HashMap::new();
        for t in arr.iter() {
            let sender = t.get_transaction().get_source();
            let sequence_number = t.get_transaction().get_nonce();
            match map.entry(sender.to_string()) {
                std::collections::hash_map::Entry::Vacant(v) => {
                    let mut arr = Vec::new();
                    arr.push(sequence_number);
                    v.insert(arr);
                }
                std::collections::hash_map::Entry::Occupied(mut v) => {
                    v.get_mut().push(sequence_number);
                }
            }
        }
        map
    }

    /// Fetches next block of transactions for consensus.
    /// `batch_size` - size of requested block.
    /// `seen_txns` - transactions that were sent to Consensus but were not committed yet,
    ///  mempool should filter out such transactions.
    #[allow(clippy::explicit_counter_loop)]
    pub fn get_block(
        &self,
        batch_size: u64,
        max_contract_size: u64,
        exclude_transactions: &HashMap<String, CommittedTransaction>,
    ) -> Vec<TransactionSignRaw> {
        let mut txn_walked = 0u64;
        let mut priority_index = PriorityIndex::new();
        let iter_queue =
            self.transactions
                .iter_queue(&mut priority_index, &self.seq_cache, max_contract_size);

        let mut block: Vec<TransactionSignRaw> = Vec::with_capacity(batch_size as usize);
        for k in iter_queue {
            if let Some(t) = self.transactions.get(&k.address, k.seq) {
                // exclude commited tx
                if let Some(v) = exclude_transactions.get(&k.address) {
                    if k.seq <= v.max_seq {
                        continue;
                    }
                }

                txn_walked += 1;
                block.push(t);
                if txn_walked >= batch_size {
                    break;
                }
            }
        }

        block
    }

    /// Fetches next block of transactions hash list for consensus.
    /// `batch_size` - size of requested block.
    /// `seen_txns` - transactions that were sent to Consensus but were not committed yet,
    ///  mempool should filter out such transactions.
    #[allow(clippy::explicit_counter_loop)]
    pub fn get_block_txhashs(
        &self,
        batch_size: u64,
        max_contract_size: u64,
        _ledger_seq: i64,
        exclude_transactions: &HashMap<String, CommittedTransaction>,
    ) -> Vec<Vec<u8>> {
        let mut txn_walked = 0u64;

        let mut priority_index = PriorityIndex::new();
        let iter_queue =
            self.transactions
                .iter_queue(&mut priority_index, &self.seq_cache, max_contract_size);
        let mut block: Vec<Vec<u8>> = Vec::with_capacity(batch_size as usize);
        for k in iter_queue {
            if let Some(hash) = self.transactions.get_hash(&k.address, k.seq) {
                // exclude commited tx
                if let Some(v) = exclude_transactions.get(&k.address) {
                    if k.seq <= v.max_seq {
                        continue;
                    }
                }

                txn_walked += 1;
                block.push(hash);
                if txn_walked >= batch_size {
                    break;
                }
            }
        }

        block
    }

    pub fn get_block_by_hashs(
        &self,
        hash_list: &[Vec<u8>],
        _ledger_seq: i64,
    ) -> (Vec<TransactionSignRaw>, HashMap<Vec<u8>, usize>) {
        let mut block: Vec<TransactionSignRaw> = Vec::with_capacity(hash_list.len());
        let mut lacktxs: HashMap<Vec<u8>, usize> = HashMap::new();
        for (index, hash) in hash_list.iter().enumerate() {
            match self.transactions.get_by_hash(hash) {
                Some(t) => {
                    block.push(t.clone());
                }
                None => {
                    block.push(TransactionSignRaw::default());
                    lacktxs.insert(hash.clone(), index);
                }
            }
        }

        (block, lacktxs)
    }

    /// Periodic core pool garbage collection.
    /// Removes all expired transactions and clears expired entries in metrics
    /// cache and sequence number cache.
    pub(crate) fn gc(&mut self) {
        self.transactions.gc();
    }
}
