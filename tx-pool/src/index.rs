use std::{
    cmp::Ordering,
    collections::{btree_set, BTreeMap, BTreeSet},
    iter::Rev,
    time::Duration,
};

use crate::transaction::PoolTransaction;

pub type AccountTransactions = BTreeMap<u64, PoolTransaction>;

/// PriorityIndex represents the main Priority Queue in pool.
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

    pub(crate) fn insert(&mut self, txn: &PoolTransaction) {
        self.data.insert(self.make_key(txn));
    }

    fn make_key(&self, txn: &PoolTransaction) -> OrderedQueueKey {
        OrderedQueueKey {
            gas_ranking_score: txn.get_gas_price(),
            expiration_time: txn.get_expiration_time(),
            address: txn.get_sender().to_string(),
            seq: txn.get_seq(),
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
    pub seq: u64,
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
        self.seq.cmp(&other.seq).reverse()
    }
}

/// TTLIndex is used to perform garbage collection of old transactions in pool.
/// Periodically separate GC-like job queries this index to find out transactions that have to be
/// removed. Index is represented as `BTreeSet<TTLOrderingKey>`, where `TTLOrderingKey`
/// is a logical reference to TxnInfo.
/// Index is ordered by `TTLOrderingKey::expiration_time`.
pub struct TTLIndex {
    data: BTreeSet<TTLOrderingKey>,
    get_expiration_time: Box<dyn Fn(&PoolTransaction) -> Duration + Send + Sync>,
}

impl TTLIndex {
    pub(crate) fn new<F>(get_expiration_time: Box<F>) -> Self
    where
        F: Fn(&PoolTransaction) -> Duration + 'static + Send + Sync,
    {
        Self {
            data: BTreeSet::new(),
            get_expiration_time,
        }
    }

    pub(crate) fn insert(&mut self, txn: &PoolTransaction) {
        self.data.insert(self.make_key(txn));
    }

    pub(crate) fn remove(&mut self, txn: &PoolTransaction) {
        self.data.remove(&self.make_key(txn));
    }

    /// Garbage collect all old transactions.
    pub(crate) fn gc(&mut self, now: Duration) -> Vec<TTLOrderingKey> {
        let ttl_key = TTLOrderingKey {
            expiration_time: now,
            address: String::from("000000000"),
            seq: 0,
        };

        let mut active = self.data.split_off(&ttl_key);
        let ttl_transactions = self.data.iter().cloned().collect();
        self.data.clear();
        self.data.append(&mut active);
        ttl_transactions
    }

    fn make_key(&self, txn: &PoolTransaction) -> TTLOrderingKey {
        TTLOrderingKey {
            expiration_time: (self.get_expiration_time)(txn),
            address: txn.get_sender().to_string(),
            seq: txn.get_seq(),
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
    pub seq: u64,
}

/// Be very careful with this, to not break the partial ordering.
/// See:  https://rust-lang.github.io/rust-clippy/master/index.html#derive_ord_xor_partial_ord
#[allow(clippy::derive_ord_xor_partial_ord)]
impl Ord for TTLOrderingKey {
    fn cmp(&self, other: &TTLOrderingKey) -> Ordering {
        match self.expiration_time.cmp(&other.expiration_time) {
            Ordering::Equal => (&self.address, self.seq).cmp(&(&other.address, other.seq)),
            ordering => ordering,
        }
    }
}

pub type TxIndex = (String, u64);

impl From<&PoolTransaction> for TxIndex {
    fn from(transaction: &PoolTransaction) -> Self {
        (transaction.get_sender().to_string(), transaction.get_seq())
    }
}

impl From<&OrderedQueueKey> for TxIndex {
    fn from(key: &OrderedQueueKey) -> Self {
        (key.address.clone(), key.seq)
    }
}
