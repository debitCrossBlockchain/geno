// Copyright (c) The  Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Tasks that are executed by coordinators (short-lived compared to coordinators)

use crate::coordinator::{broadcast_transaction, coordinator, gc_coordinator};
use crate::{CoreMempool, TimelineState, TxState, TxnPointer};
use crate::mempool_status::{MempoolStatus, MempoolStatusCode};
use crate::tx_validator::{
    get_account_nonce_banace, DiscardedVMStatus, TransactionValidation,
};
use crate::types::{
    notify_subscribers, MempoolConsensusRequest, MempoolConsensusResponse, SharedMempool,
    SharedMempoolNotification, SubmissionStatusBundle, TransactionSummary,MempoolConsensusReceiver,MempoolBroadCastTxReceiver,
    MempoolCommitNotificationReceiver,MempoolClientReceiver,MempoolCommitNotification
};
use crate::TEST_TXPOOL_INCHANNEL_AND_SWPAN;
use anyhow::Result;
use chrono::Local;
use futures::task::Spawn;
use futures::{channel::oneshot, stream::FuturesUnordered};
use parking_lot::{Mutex, Once, RawRwLock, RwLock};
use protobuf::Message;
use protos::ledger::TransactionSign;
use rayon::prelude::*;
use types::TransactionSignRaw;
use std::{
    cmp,
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use crate::tx_validator::TxValidator;
use tracing::*;
use utils::timing::Timestamp;
use tokio::runtime::{Builder, Handle, Runtime};
use crate::TxPoolInstanceRef;
use network::PeerNetwork;
pub type SubmissionStatus = (MempoolStatus, Option<DiscardedVMStatus>);

/// Processes transactions directly submitted by client.
pub(crate) async fn process_client_transaction_submission<V>(
    smp: SharedMempool<V>,
    mut transaction: TransactionSignRaw,
    callback: oneshot::Sender<Result<SubmissionStatus>>,
) where
    V: TransactionValidation,
{
    let statuses = process_incoming_transactions(&smp, vec![transaction], TxState::NotReady);

    if let Some(status) = statuses.get(0) {
        if callback.send(Ok(status.1.clone())).is_err() {
            // counters::CLIENT_CALLBACK_FAIL.inc();
        }
    }
}

fn is_txn_retryable(result: SubmissionStatus) -> bool {
    result.0.code == MempoolStatusCode::MempoolIsFull
}

/// Submits a list of SignedTransaction to the local mempool
/// and returns a vector containing AdmissionControlStatus.
pub(crate) fn process_incoming_transactions<V>(
    smp: &SharedMempool<V>,
    transactions: Vec<TransactionSignRaw>,
    tx_state: TxState,
) -> Vec<SubmissionStatusBundle>
where
    V: TransactionValidation,
{
    let mut statuses = vec![];
    if TEST_TXPOOL_INCHANNEL_AND_SWPAN {
        for txn in transactions {
            let status = MempoolStatus::new(MempoolStatusCode::Accepted);
            let tx_hash = txn.tx.hash();
            let hash = String::from_utf8(Vec::from(tx_hash)).unwrap();
            let result = status.with_message(hash);
            statuses.push((txn, (result, None)));
        }
        return statuses;
    }

    let start = Instant::now();
    let start_storage_read = Instant::now();
    let tx_size = transactions.len();

    // Track latency: fetching seq number
    let nonce_and_banace_vec = transactions
        .par_iter()
        .map(|t| {
            get_account_nonce_banace(t.tx.sender()).map_err(|e| {
                error!("TransactionValidation get account error");
                e
            })
        })
        .collect::<Vec<_>>();

    let storage_read_latency = start_storage_read.elapsed();
    // counters::PROCESS_TXN_BREAKDOWN_LATENCY
    //     .with_label_values(&[counters::FETCH_SEQ_NUM_LABEL])
    //     .observe(storage_read_latency.as_secs_f64() / transactions.len() as f64);

    let transactions: Vec<_> = transactions
        .into_iter()
        .enumerate()
        .filter_map(|(idx, t)| {
            if let Ok((db_sequence_number, banace)) = nonce_and_banace_vec[idx] {
                if t.tx.nonce() > db_sequence_number {
                    //check balance for limit fee
                    if utils::general::fees_config().consume_gas {
                        if t.tx.gas_limit() > banace {
                            statuses.push((
                                t,
                                (
                                    MempoolStatus::new(MempoolStatusCode::VmError),
                                    Some(
                                        DiscardedVMStatus::INSUFFICIENT_BALANCE_FOR_TRANSACTION_FEE,
                                    ),
                                ),
                            ));
                        } else {
                            return Some((t, db_sequence_number));
                        }
                    } else {
                        return Some((t, db_sequence_number));
                    }
                } else {
                    statuses.push((
                        t,
                        (
                            MempoolStatus::new(MempoolStatusCode::VmError),
                            Some(DiscardedVMStatus::SEQUENCE_NUMBER_TOO_OLD),
                        ),
                    ));
                }
            } else {
                // Failed to get transaction
                statuses.push((
                    t,
                    (
                        MempoolStatus::new(MempoolStatusCode::VmError),
                        Some(DiscardedVMStatus::RESOURCE_DOES_NOT_EXIST),
                    ),
                ));
            }
            None
        })
        .collect();

    // Track latency: VM validation
    let start_verify_sign = Instant::now();
    let validation_results = transactions
        .par_iter()
        .map(|t| smp.validator.read().validate_transaction(&t.0))
        .collect::<Vec<_>>();
    {
        let mut mempool = smp.mempool.write();
        for (idx, (mut transaction, db_sequence_number)) in transactions.into_iter().enumerate() {
            if let Ok(validation_result) = &validation_results[idx] {
                match validation_result.status() {
                    None => {
                        let gas_amount = transaction.tx.gas_limit();
                        let ranking_score = validation_result.score();
                        let mempool_status = mempool.add_txn(
                            transaction.clone(),
                            gas_amount,
                            ranking_score,
                            db_sequence_number,
                            tx_state,
                        );
                        statuses.push((transaction, (mempool_status, None)));
                    }
                    Some(validation_status) => {
                        statuses.push((
                            transaction.clone(),
                            (
                                MempoolStatus::new(MempoolStatusCode::VmError),
                                Some(validation_status),
                            ),
                        ));
                    }
                }
            }
        }

        // insert_tx_timer.stop_and_record();
    }

    statuses
}

/// Processes transactions from other nodes.
pub(crate) async fn process_transaction_broadcast<V>(
    smp: SharedMempool<V>,
    transactions: Vec<TransactionSignRaw>,
) where
    V: TransactionValidation,
{ 
    let results = process_incoming_transactions(&smp, transactions, TxState::NotReady);
}

/// Remove transactions that are committed (or rejected) so that we can stop broadcasting them.
pub(crate) async fn process_committed_transactions<V>(
    smp: SharedMempool<V>,
    msg: MempoolCommitNotification,
    block_timestamp_usecs: u64,
    is_rejected: bool,
) where
    V: TransactionValidation,
{
    let tx_size = msg.transactions.len();
    let mempool = &smp.mempool;
    let start = Instant::now();
    msg.transactions
        .par_iter()
        .for_each(|(sender, transaction)| {
            mempool
                .write()
                .remove_transaction(sender, transaction.max_seq, is_rejected);
        });

    info!(
        "[tx-pool] txpool-trace process_committed_transactions txs({}) use({})micros",
        tx_size,
        start.elapsed().as_micros()
    );
}

pub(crate) fn process_consensus_request<V: TransactionValidation>(
    smp: &SharedMempool<V>,
    req: MempoolConsensusRequest,
) {
    let (resp, callback) = match req {
        MempoolConsensusRequest::GetBlockRequest(
            max_block_size,
            max_contract_size,
            transactions,
            callback,
        ) => {
            let exclude_transactions: HashSet<TxnPointer> = transactions
                .iter()
                .map(|txn| (txn.sender.clone(), txn.sequence_number))
                .collect();
            let mut txns;
            {
                let mempool = smp.mempool.write();
                // gc before pulling block as extra protection against txns that may expire in consensus
                // Note: this gc operation relies on the fact that consensus uses the system time to determine block timestamp
                // let curr_time = diem_infallible::duration_since_epoch();
                // mempool.gc_by_expiration_time(curr_time);
                let block_size = cmp::max(max_block_size, 1);
                txns = mempool.get_block(block_size, max_contract_size, &HashMap::new());
            }

            let pulled_block = txns.drain(..).map(TransactionSignRaw::into).collect();
            (
                MempoolConsensusResponse::GetBlockResponse(pulled_block),
                callback,
            )
        }
        MempoolConsensusRequest::RejectNotification(transactions, callback) => {
            (MempoolConsensusResponse::CommitResponse(), callback)
        }
    };

    if callback.send(Ok(resp)).is_err() {
        error!("process_consensus_request callback send error");
    }
}


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
    validator: Arc<RwLock<V>>,
) where
    V: TransactionValidation + 'static,
{
    let smp = SharedMempool {
        mempool: mempool.clone(),
        config: config.clone(), 
        validator,
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
    let vm_validator = Arc::new(RwLock::new(TxValidator::new()));
    start_shared_mempool(
        runtime.handle(),
        config,
        mempool,
        client_events,
        broadcast_tx_events,
        consensus_requests,
        committed_events,
        vm_validator,
        // vec![],
    );
    runtime
}