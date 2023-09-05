// Copyright (c) The  Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::shared_mempool::coordinator::{broadcast_transaction, coordinator, gc_coordinator};
use crate::shared_mempool::temp_db::DbReader;
use crate::shared_mempool::tx_pool_config::TxPoolConfig;
use crate::shared_mempool::tx_validator::{TransactionValidation, TxValidator};
use crate::TxPoolInstanceRef;
use crate::{
    core_mempool::CoreMempool,
    shared_mempool::types::{
        MempoolBroadCastTxReceiver, MempoolClientReceiver, MempoolCommitNotificationReceiver,
        MempoolConsensusReceiver, SharedMempool, SharedMempoolNotification, SubmissionStatus,
    },
};
use anyhow::Result;
use futures::channel::{
    mpsc::{self, UnboundedReceiver, UnboundedSender},
    oneshot,
};
use network::PeerNetwork;
use parking_lot::{Mutex, Once, RawRwLock, RwLock};
use protos::ledger::TransactionSign;
use std::{collections::HashMap, sync::Arc};
use tokio::runtime::{Builder, Handle, Runtime};
/// Bootstrap of SharedMempool.
/// Creates a separate Tokio Runtime that runs the following routines:
///   - outbound_sync_task (task that periodically broadcasts transactions to peers).
///   - inbound_network_task (task that handles inbound mempool messages and network events).
///   - gc_task (task that performs GC of all expired transactions by SystemTTL).
pub(crate) fn start_shared_mempool<V>(
    executor: &Handle,
    config: &configure::TxPoolConfig,
    mempool: Arc<RwLock<CoreMempool>>,
    client_events: MempoolClientReceiver,
    broadcast_tx_events: MempoolBroadCastTxReceiver,
    consensus_requests: MempoolConsensusReceiver,
    committed_events: MempoolCommitNotificationReceiver,
    db: Arc<dyn DbReader>,
    validator: Arc<RwLock<V>>,
) where
    V: TransactionValidation + 'static,
{
    // let peer_manager = Arc::new(PeerManager::new(config.base.role, config.mempool.clone()));
    //
    // let mut all_network_events = vec![];
    // let mut network_senders = HashMap::new();
    // for (network_id, network_sender, network_events) in mempool_network_handles.into_iter() {
    //     all_network_events.push((network_id.clone(), network_events));
    //     network_senders.insert(network_id, network_sender);
    // }

    let smp = SharedMempool {
        mempool: mempool.clone(),
        config: config.clone(),
        // network_senders,
        db,
        validator,
        // peer_manager,
        // subscribers,
    };

    executor.spawn(coordinator(
        smp,
        executor.clone(),
        client_events,
        broadcast_tx_events,
        consensus_requests,
        committed_events,
    ));

    executor.spawn(gc_coordinator(
        mempool.clone(),
        config.system_transaction_gc_interval_ms,
    ));

    executor.spawn(broadcast_transaction(
        mempool.clone(),
        config.broadcast_transaction_interval_ms,
    ));
}

pub fn bootstrap(
    config: &configure::TxPoolConfig,
    db: Arc<dyn DbReader>,
    client_events: MempoolClientReceiver,
    broadcast_tx_events: MempoolBroadCastTxReceiver,
    consensus_requests: MempoolConsensusReceiver,
    committed_events: MempoolCommitNotificationReceiver,
    network: PeerNetwork,
) -> Runtime {
    let runtime = Builder::new_multi_thread()
        .thread_name("shared-mem")
        .enable_all()
        .build()
        .expect("[shared mempool] failed to create runtime");
    let mempool = TxPoolInstanceRef.clone();
    {
        mempool.write().reinit(config, network);
    }
    let vm_validator = Arc::new(RwLock::new(TxValidator::new(Arc::clone(&db))));
    start_shared_mempool(
        runtime.handle(),
        config,
        mempool,
        client_events,
        broadcast_tx_events,
        consensus_requests,
        committed_events,
        db,
        vm_validator,
        // vec![],
    );
    runtime
}
