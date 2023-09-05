use configure::TxPoolConfig;
use criterion::*;
use network::PeerNetwork;
use parking_lot::{Mutex, Once, RawRwLock, RwLock};
use std::{
    cmp::max,
    collections::HashSet,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tx_pool::core_mempool::{
    index::TxnPointer,
    transaction::{MempoolTransaction, TimelineState},
    transaction_store::TransactionStore,
    ttl_cache::TtlCache,
    CoreMempool,
};
use utils::{private_key, transaction_factory::*};

fn create_txs() -> Vec<TransactionSign> {
    let sender = "did:gdt:0xf6b02a2d47b84e845b7e3623355f041bcb36daf1";
    let private_key = "fc5a55e22797ed20e78b438d9e3ca873877a7b55a604dfa7531c300e743c5ef1";

    let dest_addr = "did:gdt:0xe1ba3068fe19fd3019cb82982fca87835fbccd1f";

    let mut vec = Vec::new();
    for nonce in 1..10000 {
        let transaction = generate_pay_coin_transaction(sender, private_key, nonce, dest_addr);
        vec.push(transaction);
    }
    vec
}

fn txpool_insert(c: &mut Criterion) {
    let txs = create_txs();
    let config = TxPoolConfig::default();

    let mempool = Arc::new(RwLock::new(CoreMempool::new(
        &config,
        PeerNetwork::default(),
    )));
}

criterion_group! {
    name = txpool_benchmark;
    config = Criterion::default();
    targets = txpool_insert
}

criterion_main!(txpool_benchmark);
