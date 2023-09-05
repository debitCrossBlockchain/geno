// Copyright (c) The  Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Mempool is used to track transactions which have been submitted but not yet
//! agreed upon.
use crate::core_mempool::CoreMempool;
use crate::shared_mempool::account_address::AccountAddress;
use crate::shared_mempool::mempool_status::{MempoolStatus, MempoolStatusCode};
use crate::shared_mempool::tx_pool_config::TxPoolConfig;
use crate::shared_mempool::types::CommittedTransaction;
use crate::{
    core_mempool::{
        index::{PriorityIndex, TxnPointer},
        transaction::{MempoolTransaction, TimelineState, TxState},
        transaction_store::TransactionStore,
        ttl_cache::TtlCache,
    },
    // counters,
    logging::{LogEntry, LogSchema, TxnsLog},
};
use network::PeerNetwork;
use protobuf::{Message, RepeatedField};
use protos::common::{ProtocolsMessage, ProtocolsMessageType};
use protos::ledger::{Transaction, TransactionSign, TransactionSignSet};
use std::collections::HashMap;
use std::{
    cmp::max,
    collections::HashSet,
    time::{Duration, SystemTime},
};
use std::{net::SocketAddr, str::FromStr};
use tracing::*;
use types::TransactionSignRaw;
use utils::timing::duration_since_epoch;
pub struct Mempool {
    // Stores the metadata of all transactions in mempool (of all states).
    transactions: TransactionStore,

    sequence_number_cache: HashMap<String, u64>,
    // For each transaction, an entry with a timestamp is added when the transaction enters mempool.
    // This is used to measure e2e latency of transactions in the system, as well as the time it
    // takes to pick it up by consensus.
    pub(crate) metrics_cache: TtlCache<(String, u64), SystemTime>,
    pub system_transaction_timeout: Duration,

    // for broadcast transactions
    pub broadcast_max_batch_size: usize,
    pub broadcast_cache: Vec<TransactionSignRaw>,
    pub network: Option<PeerNetwork>,

    // for test performance
    pub count: u64,
    pub duration: Duration,
    pub read_account_duration: Duration,
    pub verify_duration: Duration,
    pub add_txn_duration: Duration,

    //for commit to delete tx
    pub waiting_be_delete: Vec<(String, u64)>,
}

impl Mempool {
    pub fn new(config: &configure::TxPoolConfig, network: Option<PeerNetwork>) -> Self {
        Mempool {
            transactions: TransactionStore::new(&config),
            sequence_number_cache: HashMap::with_capacity(config.capacity),
            metrics_cache: TtlCache::new(config.capacity, Duration::from_secs(100)),
            system_transaction_timeout: Duration::from_secs(config.system_transaction_timeout_secs),
            broadcast_cache: Vec::new(),
            broadcast_max_batch_size: config.broadcast_max_batch_size,
            network,
            count: 0,
            duration: Duration::new(0, 0),
            read_account_duration: Duration::new(0, 0),
            verify_duration: Duration::new(0, 0),
            add_txn_duration: Duration::new(0, 0),
            waiting_be_delete: Vec::new(),
        }
    }

    pub fn reinit(&mut self, config: &configure::TxPoolConfig, network: PeerNetwork) {
        self.transactions = TransactionStore::new(&config);
        self.sequence_number_cache = HashMap::with_capacity(config.capacity);
        self.broadcast_max_batch_size = config.broadcast_max_batch_size;
        self.metrics_cache = TtlCache::new(config.capacity, Duration::from_secs(100));
        self.system_transaction_timeout =
            Duration::from_secs(config.system_transaction_timeout_secs);
        self.network = Some(network);
    }

    pub fn get_by_hash(&self, hash: &[u8]) -> Option<TransactionSignRaw> {
        return self.transactions.get_by_hash(hash);
    }

    pub fn get_bench_by_hash(&self, hash_list: &[Vec<u8>]) -> (Vec<TransactionSignRaw>, Vec<Vec<u8>>) {
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
    pub(crate) fn remove_transaction(
        &mut self,
        sender: &str,
        sequence_number: u64,
        is_rejected: bool,
    ) {
        // self.metrics_cache
        //     .remove(&(sender.to_string(), sequence_number));

        let current_seq_number = self
            .sequence_number_cache
            .remove(&sender.to_string())
            .unwrap_or_default();

        // if is_rejected {
        //     if sequence_number >= current_seq_number {
        //         self.transactions
        //             .reject_transaction(&sender.to_string(), sequence_number);
        //     }
        // } else
        {
            // update current cached sequence number for account
            let new_seq_number = max(current_seq_number, sequence_number);
            self.sequence_number_cache
                .insert(sender.to_string(), new_seq_number);

            let new_seq_number = sequence_number;
            self.transactions
                .commit_transaction(sender, new_seq_number + 1);
        }
    }

    /// Used to add a transaction to the Mempool.
    /// Performs basic validation: checks account's sequence number.
    // pub(crate) fn add_txn(
    pub fn add_txn(
        &mut self,
        txn: TransactionSignRaw,
        gas_amount: u64,
        ranking_score: u128,
        db_sequence_number: u64,
        tx_state: TxState,
    ) -> MempoolStatus {
        ///todo log transaction
        let cached_value = self.sequence_number_cache.get(txn.tx.sender());
        let sequence_number =
            cached_value.map_or(db_sequence_number, |value| max(*value, db_sequence_number));
        self.sequence_number_cache
            .insert(txn.tx.sender().to_string(), sequence_number);

        // don't accept old transactions (e.g. seq is less than account's current seq_number)
        if txn.tx.nonce() <= db_sequence_number {
            return MempoolStatus::new(MempoolStatusCode::InvalidSeqNumber).with_message(format!(
                "transaction sequence number is {}, account sequence number is  {}",
                txn.tx.nonce(),
                db_sequence_number
            ));
        }

        let expiration_time = duration_since_epoch() + self.system_transaction_timeout;

        let txn_info = MempoolTransaction::new(
            txn.clone(),
            expiration_time,
            tx_state,
            db_sequence_number,
        );

        let status = self.transactions.insert(txn_info);

        if status.code == MempoolStatusCode::Accepted
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
                    //vec_signs.push(<TransactionSignRaw as TryInto<TransactionSign>>::try_into(it).unwrap());
                }
                broadcast.set_transactions(RepeatedField::from(vec_signs));
            } else {
                let vec = self
                    .broadcast_cache
                    .drain(0..self.broadcast_max_batch_size)
                    .collect::<Vec<_>>();
                let mut vec_signs: Vec<TransactionSign> = Vec::new();
                for it in vec {
                    //vec_signs.push(<TransactionSignRaw as TryInto<TransactionSign>>::try_into(it).unwrap());
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

        let t1 = chrono::Local::now();
        let mut priority_index = PriorityIndex::new();
        let iter_queue = self.transactions.iter_queue(
            &mut priority_index,
            &self.sequence_number_cache,
            max_contract_size,
        );
        let iter_queue_size = iter_queue.len();
        let t2 = chrono::Local::now();

        // let block: Vec<_> = iter_queue
        //     .filter_map(|k| self.transactions.get(&k.address, k.sequence_number))
        //     .collect();

        let mut block: Vec<TransactionSignRaw> = Vec::with_capacity(batch_size as usize);
        for k in iter_queue {
            if let Some(t) = self.transactions.get(&k.address, k.sequence_number) {
                // exclude commited tx
                if let Some(v) = exclude_transactions.get(&k.address) {
                    if k.sequence_number <= v.max_seq {
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

        // let t3 = chrono::Local::now();
        // info!(
        //     "[tx-pool] txpool-trace get_block txs({}) total use({:?})micros ({:?}) + ({:?}) iter_queue({}) {} {}",
        //     block.len(),
        //     (t3 - t1).num_microseconds(),
        //     (t2 - t1).num_microseconds(),
        //     (t3 - t2).num_microseconds(),
        //     iter_queue_size,
        //     counters::MAINLOOP_TPS.read().summary(),
        //     counters::INSERT_PROCESS_TPS.read().summary()
        // );
        // {
        //     counters::MAINLOOP_TPS.write().start();
        // }
        // {
        //     counters::INSERT_PROCESS_TPS.write().start();
        // }
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
        ledger_seq: i64,
        exclude_transactions: &HashMap<String, CommittedTransaction>,
    ) -> Vec<Vec<u8>> {
        let mut txn_walked = 0u64;

        let t1 = chrono::Local::now();
        let mut priority_index = PriorityIndex::new();
        let iter_queue = self.transactions.iter_queue(
            &mut priority_index,
            &self.sequence_number_cache,
            max_contract_size,
        );
        let iter_queue_size = iter_queue.len();
        let t2 = chrono::Local::now();

        // let block: Vec<_> = iter_queue
        //     .filter_map(|k| self.transactions.get_hash(&k.address, k.sequence_number))
        //     .collect();

        let mut block: Vec<Vec<u8>> = Vec::with_capacity(batch_size as usize);
        for k in iter_queue {
            if let Some(hash) = self.transactions.get_hash(&k.address, k.sequence_number) {
                // exclude commited tx
                if let Some(v) = exclude_transactions.get(&k.address) {
                    if k.sequence_number <= v.max_seq {
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

        // let t3 = chrono::Local::now();
        // info!(
        //     "[tx-pool] txpool-trace get_block_txhashs ledger_seq({}) txs({}) total use({:?})micros ({:?}) + ({:?}) total {} iter_queue({}) {} {}",
        //     ledger_seq,block.len(),
        //     (t3 - t1).num_microseconds(),
        //     (t2 - t1).num_microseconds(),
        //     (t3 - t2).num_microseconds(),
        //     self.transactions.count(),
        //     iter_queue_size,
        //     counters::MAINLOOP_TPS.read().summary(),
        //     counters::INSERT_PROCESS_TPS.read().summary()
        // );
        // {
        //     counters::MAINLOOP_TPS.write().start();
        // }
        // {
        //     counters::INSERT_PROCESS_TPS.write().start();
        // }
        block
    }

    pub fn get_block_by_hashs(
        &self,
        hash_list: &[Vec<u8>],
        ledger_seq: i64,
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

    /// Periodic core mempool garbage collection.
    /// Removes all expired transactions and clears expired entries in metrics
    /// cache and sequence number cache.
    pub(crate) fn gc(&mut self) {
        let start = std::time::Instant::now();
        let now = SystemTime::now();
        self.transactions.gc_by_system_ttl(&self.metrics_cache);
        // self.metrics_cache.gc(now);
        let latency = start.elapsed();
        info!("[tx-pool] txpool-trace gc({})micros", latency.as_micros());
    }

    pub(crate) fn statistic(
        &mut self,
        count: u64,
        duration: Duration,
        read_account_duration: Duration,
        verify_duration: Duration,
        add_txn_duration: Duration,
    ) {
        self.count += count;
        self.duration += duration;
        self.read_account_duration += read_account_duration;
        self.verify_duration += verify_duration;
        self.add_txn_duration += add_txn_duration;
        // if self.count >= 10000 {
        //     info!(
        //         "[tx-pool] txpool-trace statistic insert count {} avg insert {} micros({}-{}-{})",
        //         self.count,
        //         self.duration.as_micros() / (self.count as u128),
        //         self.read_account_duration.as_micros() / (self.count as u128),
        //         self.verify_duration.as_micros() / (self.count as u128),
        //         self.add_txn_duration.as_micros() / (self.count as u128)
        //     );
        //     self.count = 0;
        //     self.duration = Duration::new(0, 0);
        //     self.read_account_duration = Duration::new(0, 0);
        //     self.verify_duration = Duration::new(0, 0);
        //     self.add_txn_duration = Duration::new(0, 0);
        // }
    }

    pub(crate) fn display_statistic(&self) -> String {
        format!(
            "insert count {} avg insert {} micros({}-{}-{})",
            self.count,
            self.duration.as_micros() / (self.count as u128),
            self.read_account_duration.as_micros() / (self.count as u128),
            self.verify_duration.as_micros() / (self.count as u128),
            self.add_txn_duration.as_micros() / (self.count as u128)
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::core_mempool::{
        index::TxnPointer,
        transaction::{self, MempoolTransaction, TimelineState, TxState},
        transaction_store::TransactionStore,
        ttl_cache::TtlCache,
        CoreMempool,
    };
    use configure::TxPoolConfig;
    use criterion::*;
    use network::PeerNetwork;
    use parking_lot::{Mutex, Once, RawRwLock, RwLock};
    use protos::ledger::TransactionSign;
    use std::{
        cmp::max,
        collections::HashSet,
        sync::Arc,
        time::{Duration, Instant, SystemTime},
    };
    use utils::{private_key, transaction_factory::*};

    fn create_txs() -> Vec<TransactionSign> {
        let sender = "did:gdt:0xf6b02a2d47b84e845b7e3623355f041bcb36daf1";
        let private_key = "fc5a55e22797ed20e78b438d9e3ca873877a7b55a604dfa7531c300e743c5ef1";

        let dest_addr = "did:gdt:0xe1ba3068fe19fd3019cb82982fca87835fbccd1f";

        let mut vec = Vec::new();
        for nonce in 1..=10000 {
            let transaction =
                generate_pay_coin_transaction(sender, private_key, nonce, dest_addr, "abc", "bbc");
            vec.push(transaction);
        }
        vec
    }

    #[actix_rt::test]
    async fn test_txpool_add_txs() {
        let txs = create_txs();
        let len = txs.len();
        let config = TxPoolConfig::default();
        let mempool = Arc::new(RwLock::new(CoreMempool::new(&config, None)));

        let mut mempool = mempool.write();
        let start = Instant::now();
        for tx in txs {
            let transaction = tx.clone();
            let mut n = 1;
            if transaction.get_transaction().get_nonce() > 100 {
                n = 100;
            }
            let s = mempool.add_txn(
                tx,
                0,
                1,
                transaction.get_transaction().get_nonce() - n,
                TxState::NotReady,
            );
            // println!("{},{}", s.code, s.message);
        }
        println!(
            "txs({}) time cost: {:?} micros",
            len,
            start.elapsed().as_micros(),
        );
    }
}
