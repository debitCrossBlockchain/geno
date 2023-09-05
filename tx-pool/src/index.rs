// Copyright (c) The  Core Contributors
// SPDX-License-Identifier: Apache-2.0

/// This module provides various indexes used by Mempool.
use crate::transaction::{MempoolTransaction, TimelineState};
// use crate::{
//     counters,
// };
use crate::account_address::AccountAddress;
use itertools::Itertools;
use rand::seq::SliceRandom;
use std::{
    cmp::Ordering,
    collections::{btree_map, btree_set, BTreeMap, BTreeSet, HashMap, HashSet},
    iter::Rev,
    ops::Bound,
    time::Duration,
};

pub type AccountTransactions = BTreeMap<u64, MempoolTransaction>;

/// PriorityIndex represents the main Priority Queue in Mempool.
/// It's used to form the transaction block for Consensus.
/// Transactions are ordered by gas price. Second level ordering is done by expiration time.
///
/// We don't store the full content of transactions in the index.
/// Instead we use `OrderedQueueKey` - logical reference to the transaction in the main store.
pub struct PriorityIndex {
    data: BTreeSet<OrderedQueueKey>,
}

pub type PriorityQueueIter<'a> = Rev<btree_set::Iter<'a, OrderedQueueKey>>;

impl PriorityIndex {
    pub(crate) fn new() -> Self {
        Self {
            data: BTreeSet::new(),
        }
    }

    pub(crate) fn insert(&mut self, txn: &MempoolTransaction) {
        self.data.insert(self.make_key(txn));
    }

    pub(crate) fn remove(&mut self, txn: &MempoolTransaction) {
        self.data.remove(&self.make_key(txn));
    }

    pub(crate) fn contains(&self, txn: &MempoolTransaction) -> bool {
        self.data.contains(&self.make_key(txn))
    }

    fn make_key(&self, txn: &MempoolTransaction) -> OrderedQueueKey {
        OrderedQueueKey {
            gas_ranking_score: txn.get_tx().tx.gas_price(),
            expiration_time: txn.get_expiration_time(),
            address: txn.get_sender().to_string(),
            sequence_number: txn.get_sequence_number(),
        }
    }

    pub(crate) fn iter(&self) -> PriorityQueueIter {
        self.data.iter().rev()
    }

    pub(crate) fn size(&self) -> usize {
        self.data.len()
    }
}

#[derive(Eq, PartialEq, Clone, Debug, Hash)]
pub struct OrderedQueueKey {
    pub gas_ranking_score: u128,
    pub expiration_time: Duration,
    pub address: String,
    pub sequence_number: u64,
}

impl PartialOrd for OrderedQueueKey {
    fn partial_cmp(&self, other: &OrderedQueueKey) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedQueueKey {
    fn cmp(&self, other: &OrderedQueueKey) -> Ordering {
        match self.gas_ranking_score.cmp(&other.gas_ranking_score) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        match self.address.cmp(&other.address) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        self.sequence_number.cmp(&other.sequence_number).reverse()
    }
}

/// TTLIndex is used to perform garbage collection of old transactions in Mempool.
/// Periodically separate GC-like job queries this index to find out transactions that have to be
/// removed. Index is represented as `BTreeSet<TTLOrderingKey>`, where `TTLOrderingKey`
/// is a logical reference to TxnInfo.
/// Index is ordered by `TTLOrderingKey::expiration_time`.
pub struct TTLIndex {
    data: BTreeSet<TTLOrderingKey>,
    get_expiration_time: Box<dyn Fn(&MempoolTransaction) -> Duration + Send + Sync>,
}

impl TTLIndex {
    pub(crate) fn new<F>(get_expiration_time: Box<F>) -> Self
    where
        F: Fn(&MempoolTransaction) -> Duration + 'static + Send + Sync,
    {
        Self {
            data: BTreeSet::new(),
            get_expiration_time,
        }
    }

    pub(crate) fn insert(&mut self, txn: &MempoolTransaction) {
        self.data.insert(self.make_key(txn));
    }

    pub(crate) fn remove(&mut self, txn: &MempoolTransaction) {
        self.data.remove(&self.make_key(txn));
    }

    /// Garbage collect all old transactions.
    pub(crate) fn gc(&mut self, now: Duration) -> Vec<TTLOrderingKey> {
        let ttl_key = TTLOrderingKey {
            expiration_time: now,
            address: AccountAddress::ZERO.to_string(),
            sequence_number: 0,
        };

        let mut active = self.data.split_off(&ttl_key);
        let ttl_transactions = self.data.iter().cloned().collect();
        self.data.clear();
        self.data.append(&mut active);
        ttl_transactions
    }

    fn make_key(&self, txn: &MempoolTransaction) -> TTLOrderingKey {
        TTLOrderingKey {
            expiration_time: (self.get_expiration_time)(txn),
            address: txn.get_sender().to_string(),
            sequence_number: txn.get_sequence_number(),
        }
    }

    pub(crate) fn size(&self) -> usize {
        self.data.len()
    }
}

#[allow(clippy::derive_ord_xor_partial_ord)]
#[derive(Eq, PartialEq, PartialOrd, Clone, Debug)]
pub struct TTLOrderingKey {
    pub expiration_time: Duration,
    pub address: String,
    pub sequence_number: u64,
}

/// Be very careful with this, to not break the partial ordering.
/// See:  https://rust-lang.github.io/rust-clippy/master/index.html#derive_ord_xor_partial_ord
#[allow(clippy::derive_ord_xor_partial_ord)]
impl Ord for TTLOrderingKey {
    fn cmp(&self, other: &TTLOrderingKey) -> Ordering {
        match self.expiration_time.cmp(&other.expiration_time) {
            Ordering::Equal => {
                (&self.address, self.sequence_number).cmp(&(&other.address, other.sequence_number))
            }
            ordering => ordering,
        }
    }
}

/// ParkingLotIndex keeps track of "not_ready" transactions, e.g., transactions that
/// can't be included in the next block because their sequence number is too high.
/// We keep a separate index to be able to efficiently evict them when Mempool is full.
pub struct ParkingLotIndex {
    // DS invariants:
    // 1. for each entry (account, txns) in `data`, `txns` is never empty
    // 2. for all accounts, data.get(account_indices.get(`account`)) == (account, sequence numbers of account's txns)
    data: HashMap<String, HashSet<u64>>,
    size: usize,
}

impl ParkingLotIndex {
    pub(crate) fn new() -> Self {
        Self {
            data: HashMap::new(),
            size: 0,
        }
    }

    pub(crate) fn insert(&mut self, txn: &MempoolTransaction) {
        let sender = txn.get_sender();
        let sequence_number = txn.get_sequence_number();
        let is_new_entry = match self.data.get_mut(sender) {
            Some(set) => set.insert(sequence_number),
            None => {
                let mut set = HashSet::new();
                set.insert(sequence_number);
                self.data.insert(sender.to_string(), set);
                true
            }
        };
        if is_new_entry {
            self.size += 1;
        }
    }

    pub(crate) fn remove(&mut self, txn: &MempoolTransaction) {
        let sender = txn.get_sender();
        let sequence_number = txn.get_sequence_number();
        if let Some(set) = self.data.get_mut(sender) {
            if set.remove(&sequence_number) {
                self.size -= 1;
            }
            if set.is_empty() {
                self.data.remove(sender);
            }
        }
    }

    pub(crate) fn contains(&self, account: &str, seq_num: &u64) -> bool {
        if let Some(set) = self.data.get(account) {
            if let Some(v) = set.get(seq_num) {
                return true;
            }
        }
        false
    }

    /// Returns a random "non-ready" transaction (with highest sequence number for that account).
    pub(crate) fn get_poppable(&mut self) -> Option<TxnPointer> {
        for (sender, set) in self.data.iter() {
            for seq in set.iter() {
                return Some((sender.clone(), *seq));
            }
        }
        None
    }

    pub(crate) fn size(&self) -> usize {
        self.size
    }
}

/// Logical pointer to `MempoolTransaction`.
/// Includes Account's address and transaction sequence number.
pub type TxnPointer = (String, u64);

impl From<&MempoolTransaction> for TxnPointer {
    fn from(transaction: &MempoolTransaction) -> Self {
        (
            transaction.get_sender().to_string(),
            transaction.get_sequence_number(),
        )
    }
}

impl From<&OrderedQueueKey> for TxnPointer {
    fn from(key: &OrderedQueueKey) -> Self {
        (key.address.clone(), key.sequence_number)
    }
}
