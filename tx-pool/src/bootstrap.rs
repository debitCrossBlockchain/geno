use crate::status::{Status, StatusCode};
use crate::tx_validator::TxValidator;
use crate::tx_validator::{get_account_nonce_banace, DiscardedVMStatus, TransactionValidation};
use crate::types::{
    BroadCastTxReceiver, ClientReceiver, CommitNotification, CommitNotificationReceiver, Shared,
    SubmissionStatusBundle,
};
use crate::TxPoolInstanceRef;
use crate::TEST_TXPOOL_INCHANNEL_AND_SWPAN;
use crate::{CoreMempool, TxState, TxnPointer};
use anyhow::Result;
use futures::future::{Future, FutureExt};
use futures::{
    channel::{mpsc, oneshot},
    stream::{select_all, FuturesUnordered},
    StreamExt,
};
use network::PeerNetwork;
use parking_lot::{Mutex, Once, RawRwLock, RwLock};
use protobuf::Message;
use protos::ledger::TransactionSign;
use rayon::prelude::*;
use std::{
    cmp,
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::runtime::{Builder, Handle, Runtime};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;
use tracing::*;
use types::TransactionSignRaw;
pub type SubmissionStatus = (Status, Option<DiscardedVMStatus>);

#[derive(Clone, Debug)]
pub struct BoundedExecutor {
    semaphore: Arc<Semaphore>,
    executor: Handle,
}

impl BoundedExecutor {
    /// Create a new `BoundedExecutor` from an existing tokio [`Handle`]
    /// with a maximum concurrent task capacity of `capacity`.
    pub fn new(capacity: usize, executor: Handle) -> Self {
        let semaphore = Arc::new(Semaphore::new(capacity));
        Self {
            semaphore,
            executor,
        }
    }

    /// Spawn a [`Future`] on the `BoundedExecutor`. This function is async and
    /// will block if the executor is at capacity until one of the other spawned
    /// futures completes. This function returns a [`JoinHandle`] that the caller
    /// can `.await` on for the results of the [`Future`].
    pub async fn spawn<F>(&self, f: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let permit = self.semaphore.clone().acquire_owned().await.unwrap();
        self.spawn_with_permit(f, permit)
    }

    /// Try to spawn a [`Future`] on the `BoundedExecutor`. If the `BoundedExecutor`
    /// is at capacity, this will return an `Err(F)`, passing back the future the
    /// caller attempted to spawn. Otherwise, this will spawn the future on the
    /// executor and send back a [`JoinHandle`] that the caller can `.await` on
    /// for the results of the [`Future`].
    pub fn try_spawn<F>(&self, f: F) -> Result<JoinHandle<F::Output>, F>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        match self.semaphore.clone().try_acquire_owned().ok() {
            Some(permit) => Ok(self.spawn_with_permit(f, permit)),
            None => Err(f),
        }
    }

    fn spawn_with_permit<F>(
        &self,
        f: F,
        spawn_permit: OwnedSemaphorePermit,
    ) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        // Release the permit back to the semaphore when this task completes.
        let f = f.map(move |ret| {
            drop(spawn_permit);
            ret
        });
        self.executor.spawn(f)
    }
}

/// Coordinator that handles inbound network events and outbound txn broadcasts.
pub(crate) async fn coordinator<V>(
    mut smp: Shared<V>,
    executor: Handle,
    mut client_events: ClientReceiver,
    mut broadcast_tx_events: BroadCastTxReceiver,
    mut committed_events: CommitNotificationReceiver,
) where
    V: TransactionValidation,
{
    // Use a BoundedExecutor to restrict only `workers_available` concurrent
    // worker tasks that can process incoming transactions.
    let workers_available = smp.config.shared_mempool_max_concurrent_inbound_syncs;
    let bounded_executor = BoundedExecutor::new(workers_available, executor.clone());

    loop {
        ::futures::select! {
            (mut transaction, callback) = client_events.select_next_some() => {
                bounded_executor.spawn(process_client_transaction_submission(smp.clone(),transaction,callback)).await;
            },
            transactions = broadcast_tx_events.select_next_some() => {
                // handle_broadcast_event(&mut smp, &bounded_executor, transactions).await;
                bounded_executor.spawn(process_transaction_broadcast(smp.clone(),transactions)).await;
            },
            msg = committed_events.select_next_some()=>{
                bounded_executor.spawn(process_committed_transactions(smp.clone(),msg, 0,false)).await;
            }
            complete => break,
        }
    }
}

/// Garbage collect all expired transactions by SystemTTL.
pub(crate) async fn gc_coordinator(mempool: Arc<RwLock<CoreMempool>>, gc_interval_ms: u64) {
    let mut interval = IntervalStream::new(interval(Duration::from_millis(gc_interval_ms)));
    while let Some(_interval) = interval.next().await {
        mempool.write().gc();
    }
}

/// broadcast transaction
pub(crate) async fn broadcast_transaction(pool: Arc<RwLock<CoreMempool>>, tx_interval: u64) {
    let mut interval = IntervalStream::new(interval(Duration::from_millis(tx_interval)));
    while let Some(_interval) = interval.next().await {
        pool.write().broadcast_transaction();
    }
}

/// Processes transactions directly submitted by client.
pub(crate) async fn process_client_transaction_submission<V>(
    smp: Shared<V>,
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

/// Submits a list of SignedTransaction to the local mempool
/// and returns a vector containing AdmissionControlStatus.
pub(crate) fn process_incoming_transactions<V>(
    smp: &Shared<V>,
    transactions: Vec<TransactionSignRaw>,
    tx_state: TxState,
) -> Vec<SubmissionStatusBundle>
where
    V: TransactionValidation,
{
    let mut statuses = vec![];
    if TEST_TXPOOL_INCHANNEL_AND_SWPAN {
        for txn in transactions {
            let status = Status::new(StatusCode::Accepted);
            let tx_hash = txn.tx.hash();
            let hash = String::from_utf8(Vec::from(tx_hash)).unwrap();
            let result = status.with_message(hash);
            statuses.push((txn, (result, None)));
        }
        return statuses;
    }

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
                                    Status::new(StatusCode::VmError),
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
                            Status::new(StatusCode::VmError),
                            Some(DiscardedVMStatus::SEQUENCE_NUMBER_TOO_OLD),
                        ),
                    ));
                }
            } else {
                // Failed to get transaction
                statuses.push((
                    t,
                    (
                        Status::new(StatusCode::VmError),
                        Some(DiscardedVMStatus::RESOURCE_DOES_NOT_EXIST),
                    ),
                ));
            }
            None
        })
        .collect();

    // Track latency: VM validation
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
                            (Status::new(StatusCode::VmError), Some(validation_status)),
                        ));
                    }
                }
            }
        }
    }

    statuses
}

/// Processes transactions from other nodes.
pub(crate) async fn process_transaction_broadcast<V>(
    smp: Shared<V>,
    transactions: Vec<TransactionSignRaw>,
) where
    V: TransactionValidation,
{
    let results = process_incoming_transactions(&smp, transactions, TxState::NotReady);
}

/// Remove transactions that are committed (or rejected) so that we can stop broadcasting them.
pub(crate) async fn process_committed_transactions<V>(
    smp: Shared<V>,
    msg: CommitNotification,
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

pub fn bootstrap(
    config: &configure::TxPoolConfig,
    client_events: ClientReceiver,
    broadcast_tx_events: BroadCastTxReceiver,
    committed_events: CommitNotificationReceiver,
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
    let validator = Arc::new(RwLock::new(TxValidator::new()));
    let executor = runtime.handle();

    let smp = Shared {
        mempool: mempool.clone(),
        config: config.clone(),
        validator,
    };

    executor.spawn(coordinator(
        smp,
        executor.clone(),
        client_events,
        broadcast_tx_events,
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

    runtime
}
