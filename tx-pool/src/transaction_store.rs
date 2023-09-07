use crate::{
    index::{AccountTransactions, ParkingLotIndex, PriorityIndex, PriorityQueueIter, TTLIndex},
    transaction::{MempoolTransaction, TxState},
    ttl_cache::TtlCache,
};
use crate::status::{Status, StatusCode};
use crate::tx_pool_config::TxPoolConfig;
use protobuf::Message;
use protos::common::ProtocolsMessageType;
use types::TransactionSignRaw;
use std::{
    collections::HashMap,
    ops::Bound,
    time::{Duration, SystemTime},
};
use tracing::info;
use utils::timing::duration_since_epoch;

/// TransactionStore is in-memory storage for all transactions in mempool.
pub struct TransactionStore {
    transactions: HashMap<String, AccountTransactions>,
    system_ttl_index: TTLIndex,

    hash_index: HashMap<Vec<u8>, (String, u64)>,
    // configuration
    capacity: usize,
    capacity_per_user: usize,
}

impl TransactionStore {
    pub(crate) fn new(config: &configure::TxPoolConfig) -> Self {
        Self {
            transactions: HashMap::new(),

            system_ttl_index: TTLIndex::new(Box::new(|t: &MempoolTransaction| {
                t.get_expiration_time()
            })),

            hash_index: HashMap::new(),

            capacity: config.capacity,
            capacity_per_user: config.capacity_per_user,
        }
    }

    pub(crate) fn get_by_hash(&self, hash: &[u8]) -> Option<TransactionSignRaw> {
        match self.hash_index.get(hash) {
            Some((address, seq)) => self.get(address, *seq),
            None => None,
        }
    }

    /// Fetch transaction by account address + sequence_number.
    pub(crate) fn get(&self, address: &str, sequence_number: u64) -> Option<TransactionSignRaw> {
        if let Some(txn) = self
            .transactions
            .get(address)
            .and_then(|txns| txns.get(&sequence_number))
        {
            return Some(txn.get_tx());
        }
        None
    }

    /// Fetch transaction hash by account address + sequence_number.
    pub(crate) fn get_hash(&self, address: &str, sequence_number: u64) -> Option<Vec<u8>> {
        if let Some(txn) = self
            .transactions
            .get(address)
            .and_then(|txns| txns.get(&sequence_number))
        {
            return Some(txn.get_hash().to_vec());
        }
        None
    }

    /// Fetch transaction hash by account address + sequence_number.
    pub(crate) fn get_tx_only_carry_hash(
        &self,
        address: &str,
        sequence_number: u64,
    ) -> Option<TransactionSignRaw> {
        if let Some(txn) = self
            .transactions
            .get(address)
            .and_then(|txns| txns.get(&sequence_number))
        {
            return Some(txn.txn.clone());
        }
        None
    }

    /// Insert transaction into TransactionStore. Performs validation checks and updates indexes.
    pub(crate) fn insert(&mut self, mut txn: MempoolTransaction) -> Status {
        let tx_hash = txn.get_hash().to_vec();
        let address = txn.get_sender().to_string();
        let sequence_info = txn.get_sequence_info();

        // check if transaction is already present in Mempool
        // e.g. given request is update
        // we allow increase in gas price to speed up process.
        // ignores the case transaction hash is same for retrying submit transaction.
        if let Some(txns) = self.transactions.get_mut(&address) {
            if let Some(current_version) = txns.get_mut(&sequence_info.transaction_sequence_number)
            {
                // already have same tx
                if current_version.get_hash() == txn.get_hash() {
                    return Status::new(StatusCode::Pending);
                }

                if current_version.get_gas_price() < txn.get_gas_price() {
                    if let Some(txn) = txns.remove(&txn.get_sequence_number()) {
                        self.index_remove(&txn);
                    }
                } else {
                    return Status::new(StatusCode::InvalidSeqNumber).with_message(
                        format!(
                            "this transacetion's nonce({}) is too old,you need update nonce,sender({}) have submitted a transaction({}) witch is same nonce",
                            sequence_info.transaction_sequence_number,
                            address,
                            String::from_utf8_lossy(current_version.get_hash().as_ref())
                        ),
                    );
                }
            }
        }

        if self.system_ttl_index.size() >= self.capacity {
            return Status::new(StatusCode::IsFull).with_message(format!(
                "mempool size: {}, capacity: {}",
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
                return Status::new(StatusCode::TooManyTransactions).with_message(
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

            self.hash_index.insert(
                txn.get_hash().to_vec(),
                (address, sequence_info.transaction_sequence_number),
            );
            self.system_ttl_index.insert(&txn);
            txns.insert(sequence_info.transaction_sequence_number, txn);
        }

        let status = Status::new(StatusCode::Accepted);
        let hash = String::from_utf8(tx_hash).unwrap();
        let result = status.with_message(hash);

        result
    }

    fn clean_committed_transactions(&mut self, address: &str, sequence_number: u64) {
        // Remove all previous seq number transactions for this account.
        // This can happen if transactions are sent to multiple nodes and one of the
        // nodes has sent the transaction to consensus but this node still has the
        // transaction sitting in mempool.
        if let Some(txns) = self.transactions.get_mut(address) {
            let mut active = txns.split_off(&sequence_number);
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
    pub(crate) fn commit_transaction(&mut self, account: &str, account_sequence_number: u64) {
        self.clean_committed_transactions(account, account_sequence_number);
        // self.process_ready_transactions(account, account_sequence_number);
    }

    pub(crate) fn reject_transaction(&mut self, account: &String, _sequence_number: u64) {
        if let Some(txns) = self.transactions.remove(account) {
            for transaction in txns.values() {
                self.index_remove(transaction);
            }
        }
    }

    /// Removes transaction from all indexes.
    fn index_remove(&mut self, txn: &MempoolTransaction) {
        self.system_ttl_index.remove(txn);
        self.hash_index.remove(txn.get_hash());
    }

    /// Garbage collect old transactions.
    pub(crate) fn gc_by_system_ttl(&mut self, metrics_cache: &TtlCache<(String, u64), SystemTime>) {
        let now = duration_since_epoch();

        self.gc(now, true, metrics_cache);
    }

    fn gc(
        &mut self,
        now: Duration,
        by_system_ttl: bool,
        metrics_cache: &TtlCache<(String, u64), SystemTime>,
    ) {
        let index = &mut self.system_ttl_index;
        let mut gc_txns = index.gc(now);
        // sort the expired txns by order of sequence number per account
        gc_txns.sort_by_key(|key| (key.address.clone(), key.sequence_number));
        let mut gc_iter = gc_txns.iter().peekable();

        while let Some(key) = gc_iter.next() {
            if let Some(txns) = self.transactions.get_mut(&key.address) {
                let park_range_start = Bound::Excluded(key.sequence_number);
                let park_range_end = gc_iter
                    .peek()
                    .filter(|next_key| key.address == next_key.address)
                    .map_or(Bound::Unbounded, |next_key| {
                        Bound::Excluded(next_key.sequence_number)
                    });

                if let Some(txn) = txns.remove(&key.sequence_number) {
                    // remove txn index
                    self.index_remove(&txn);
                    info!(
                        "gc tx {} {} {}",
                        txn.get_sender(),
                        txn.get_sequence_number(),
                        String::from_utf8(txn.get_hash().to_vec()).unwrap(),
                    );
                }
            }
        }
    }

    pub(crate) fn iter_queue<'a>(
        &'a self,
        priority_index: &'a mut PriorityIndex,
        sequence_number_cache: &HashMap<String, u64>,
        max_contract_size: u64,
    ) -> PriorityQueueIter {
        let mut tracing_seqs = HashMap::new();
        let mut contract_walked = 0u64;

        for (sender, set) in self.transactions.iter() {
            if let Some(account_sequence) = sequence_number_cache.get(sender) {
                let mut index = 0;
                for (seq, t) in set.iter() {
                    if t.state != TxState::Sended {
                        break;
                    }
                    let is_contract = t.is_contract();

                    let tx_sequence = t.get_sequence_number();
                    index += 1;

                    tracing_seqs
                        .entry(sender.clone())
                        .or_insert(*account_sequence);
                    let last_seq = tracing_seqs.get(sender).unwrap();
                    if tx_sequence != *last_seq + 1 {
                        break;
                    }
                    tracing_seqs.insert(sender.clone(), tx_sequence);
                    priority_index.insert(t);

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

    pub(crate) fn count(&self) -> usize {
        self.system_ttl_index.size()
    }

    pub(crate) fn flag_send(&mut self, txs: &HashMap<String, Vec<u64>>) {
        for (sender, sequence_list) in txs.iter() {
            if let Some(txns) = self.transactions.get_mut(sender) {
                for seq in sequence_list.iter() {
                    if let Some(memtx) = txns.get_mut(&seq) {
                        memtx.state = TxState::Sended;
                    }
                }
            }
        }
    }
}
