use crate::types::{TxPoolStatus, TxPoolStatusCode};
use crate::{
    index::{AccountTransactions, PriorityIndex, PriorityQueueIter, TTLIndex},
    transaction::{PoolTransaction, TxState},
};

use std::time::Duration;
use std::{collections::HashMap, ops::Bound};
use tracing::info;
use types::SignedTransaction;
use utils::timing::duration_since_epoch;

/// Store is in-memory storage for all transactions in pool.
pub struct Store {
    transactions: HashMap<String, AccountTransactions>,
    system_ttl_index: TTLIndex,
    index: HashMap<Vec<u8>, (String, u64)>,
    // configuration
    capacity: usize,
    capacity_per_user: usize,
}

impl Store {
    pub(crate) fn new(config: &configure::TxPoolConfig) -> Self {
        Self {
            transactions: HashMap::new(),
            system_ttl_index: TTLIndex::new(Box::new(|tx: &PoolTransaction| {
                tx.get_expiration_time()
            })),
            index: HashMap::new(),
            capacity: config.capacity,
            capacity_per_user: config.capacity_per_user,
        }
    }

    pub(crate) fn get_by_hash(&self, hash: &[u8]) -> Option<SignedTransaction> {
        match self.index.get(hash) {
            Some((address, seq)) => self.get(address, *seq),
            None => None,
        }
    }

    /// Fetch transaction by account address + sequence_number.
    pub(crate) fn get(&self, address: &str, seq: u64) -> Option<SignedTransaction> {
        if let Some(txn) = self
            .transactions
            .get(address)
            .and_then(|txns| txns.get(&seq))
        {
            return Some(txn.get_tx());
        }
        None
    }

    /// Fetch transaction hash by account address + sequence_number.
    pub(crate) fn get_hash(&self, address: &str, seq: u64) -> Option<Vec<u8>> {
        if let Some(txn) = self
            .transactions
            .get(address)
            .and_then(|txns| txns.get(&seq))
        {
            return Some(txn.get_hash().to_vec());
        }
        None
    }

    /// Insert transaction into TransactionStore. Performs validation checks and updates indexes.
    pub(crate) fn insert(&mut self, mut txn: PoolTransaction) -> TxPoolStatus {
        let address = txn.get_sender().to_string();
        let seq_info = txn.get_seq_info();

        // check if transaction is already present in pool
        // e.g. given request is update
        // we allow increase in gas price to speed up process.
        // ignores the case transaction hash is same for retrying submit transaction.
        if let Some(txns) = self.transactions.get_mut(&address) {
            if let Some(current_version) = txns.get_mut(&seq_info.tx_seq) {
                // already have same tx
                if current_version.get_hash() == txn.get_hash() {
                    return TxPoolStatus::new(TxPoolStatusCode::Accepted);
                }

                if current_version.txn.gas_limit() == txn.txn.gas_limit()
                    && current_version.txn.payload() == txn.txn.payload()
                    && current_version.txn.value() == txn.txn.value()
                    && current_version.txn.to() == txn.txn.to()
                    && current_version.get_gas_price() < txn.get_gas_price()
                {
                    if let Some(txn) = txns.remove(&txn.get_seq()) {
                        self.index_remove(&txn);
                    }
                } else {
                    return TxPoolStatus::new(TxPoolStatusCode::InvalidUpdate).with_message(
                        format!("Failed to update gas price to {}", txn.get_gas_price()),
                    );
                }
            }
        }

        if self.system_ttl_index.size() >= self.capacity {
            return TxPoolStatus::new(TxPoolStatusCode::IsFull).with_message(format!(
                "pool size: {}, capacity: {}",
                self.system_ttl_index.size(),
                self.capacity,
            ));
        }

        self.transactions
            .entry(address.to_string())
            .or_insert_with(AccountTransactions::new);

        if let Some(txns) = self.transactions.get_mut(&address) {
            // capacity check
            if txns.len() >= self.capacity_per_user {
                return TxPoolStatus::new(TxPoolStatusCode::TooManyTransactions).with_message(
                    format!(
                        "txns length: {} capacity per user: {}",
                        txns.len(),
                        self.capacity_per_user,
                    ),
                );
            }
            txn.state = TxState::Ready;
            if txn.txn.source_type == protos::ledger::TransactionSign_SourceType::P2P {
                txn.state = TxState::Sended;
            }

            self.index
                .insert(txn.get_hash().to_vec(), (address, seq_info.tx_seq));
            self.system_ttl_index.insert(&txn);
            txns.insert(seq_info.tx_seq, txn);
        }

        TxPoolStatus::new(TxPoolStatusCode::Accepted)
    }

    fn clean_committed(&mut self, address: &str, seq: u64) {
        // Remove all previous seq number transactions for this account.
        // This can happen if transactions are sent to multiple nodes and one of the
        // nodes has sent the transaction to consensus but this node still has the
        // transaction sitting in pool.
        if let Some(txns) = self.transactions.get_mut(address) {
            let mut active = txns.split_off(&seq);
            let txns_for_removal = txns.clone();
            txns.clear();
            txns.append(&mut active);

            for transaction in txns_for_removal.values() {
                self.index_remove(transaction);
            }
        }
    }

    /// Handles transaction commit.
    /// It includes deletion of all transactions with sequence number <= `account_sequence_number`
    /// and potential promotion of sequential txns to PriorityIndex/TimelineIndex.
    pub(crate) fn commit(&mut self, account: &str, seq: u64) {
        self.clean_committed(account, seq);
    }

    /// Removes transaction from all indexes.
    fn index_remove(&mut self, txn: &PoolTransaction) {
        self.system_ttl_index.remove(txn);
        self.index.remove(txn.get_hash());
    }

    pub fn gc(&mut self) {
        let now: Duration = duration_since_epoch();
        let index = &mut self.system_ttl_index;
        let mut gc_txns = index.gc(now);
        // sort the expired txns by order of sequence number per account
        gc_txns.sort_by_key(|key| (key.address.clone(), key.seq));
        let mut gc_iter = gc_txns.iter().peekable();

        while let Some(key) = gc_iter.next() {
            if let Some(txns) = self.transactions.get_mut(&key.address) {
                let _park_range_start = Bound::Excluded(key.seq);
                let _park_range_end = gc_iter
                    .peek()
                    .filter(|next_key| key.address == next_key.address)
                    .map_or(Bound::Unbounded, |next_key| Bound::Excluded(next_key.seq));

                if let Some(txn) = txns.remove(&key.seq) {
                    // remove txn index
                    self.index_remove(&txn);
                }
            }
        }
    }

    pub(crate) fn iter_queue<'a>(
        &'a self,
        priority_index: &'a mut PriorityIndex,
        seq_cache: &HashMap<String, u64>,
        max_contract_size: u64,
    ) -> PriorityQueueIter {
        let mut tracing_seqs = HashMap::new();
        let mut contract_walked = 0u64;

        for (sender, set) in self.transactions.iter() {
            if let Some(account_sequence) = seq_cache.get(sender) {
                for (seq, tx) in set.iter() {
                    if tx.state != TxState::Sended {
                        break;
                    }
                    let is_contract = tx.is_contract();
                    let tx_seq = tx.get_seq();

                    tracing_seqs
                        .entry(sender.clone())
                        .or_insert(*account_sequence);
                    let last_seq = tracing_seqs.get(sender).unwrap();
                    if tx_seq != 0{
                        if tx_seq != *last_seq + 1 {
                            break;
                        }
                    }
                    tracing_seqs.insert(sender.clone(), tx_seq);
                    priority_index.insert(tx);

                    if is_contract {
                        contract_walked += 1;
                        if contract_walked >= max_contract_size {
                            break;
                        }
                    }
                }
            }
        }
        priority_index.iter()
    }

    pub(crate) fn flag_send(&mut self, txs: &HashMap<String, Vec<u64>>) {
        for (sender, seq_list) in txs.iter() {
            if let Some(txns) = self.transactions.get_mut(sender) {
                for seq in seq_list.iter() {
                    if let Some(memtx) = txns.get_mut(&seq) {
                        memtx.state = TxState::Sended;
                    }
                }
            }
        }
    }
}
