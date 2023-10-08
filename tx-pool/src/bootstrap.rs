use crate::pool::Pool;
use crate::transaction::TxState;
use crate::types::{
    get_account_nonce_banace, BroadCastTxSender, BroadcastTxReceiver, ClientReceiver,
    CommitNotificationReceiver, CommitNotificationSender, Shared, SubmissionStatusBundle,
    TxPoolCommitNotification, TxPoolStatus, TxPoolStatusCode, TxPoolValidationStatusCode,
    Validation, Validator,
};
use crate::TX_POOL_INSTANCE_REF;
use anyhow::Result;
use futures::{
    channel::oneshot,
    future::{Future, FutureExt},
    StreamExt,
};

use network::PeerNetwork;
use parking_lot::RwLock;
use rayon::prelude::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    runtime::{Builder, Handle, Runtime},
    sync::{OwnedSemaphorePermit, Semaphore},
    task::JoinHandle,
    time::interval,
};
use tokio_stream::wrappers::IntervalStream;
use tracing::*;
use types::SignedTransaction;
pub type SubmissionStatus = (TxPoolStatus, Option<TxPoolValidationStatusCode>);

#[derive(Clone, Debug)]
pub struct BoundedExecutor {
    semaphore: Arc<Semaphore>,
    executor: Handle,
}

impl BoundedExecutor {
    /// Create a new `BoundedExecutor` from an existing tokio [`Handle`]
    /// with a maximum concurrent task capacity of `capacity`.
    pub fn new(capacity: usize, handle: Handle) -> Self {
        let semaphore = Arc::new(Semaphore::new(capacity));
        Self {
            semaphore,
            executor: handle,
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
    smp: Shared<V>,
    handle: Handle,
    mut client_events: ClientReceiver,
    mut broadcast_tx_events: BroadcastTxReceiver,
    mut committed_events: CommitNotificationReceiver,
) where
    V: Validation,
{
    // Use a BoundedExecutor to restrict only `workers_available` concurrent
    // worker tasks that can process incoming transactions.
    let workers_available = smp.config.max_concurrent_inbound_syncs;
    let executor = BoundedExecutor::new(workers_available, handle.clone());

    loop {
        ::futures::select! {
            (transaction, callback) = client_events.select_next_some() => {
                executor.spawn(process_client_submission(smp.clone(),transaction,callback)).await;
            },
            transactions = broadcast_tx_events.select_next_some() => {
                executor.spawn(process_broadcast(smp.clone(),transactions)).await;
            },
            msg = committed_events.select_next_some()=>{
                executor.spawn(process_committed(smp.clone(),msg)).await;
            }
            complete => break,
        }
    }
}

/// Garbage collect all expired transactions by SystemTTL.
pub(crate) async fn gc_coordinator(pool: Arc<RwLock<Pool>>, gc_interval_ms: u64) {
    let mut interval = IntervalStream::new(interval(Duration::from_millis(gc_interval_ms)));
    while let Some(_interval) = interval.next().await {
        pool.write().gc();
    }
}

/// broadcast transaction
pub(crate) async fn broadcast(pool: Arc<RwLock<Pool>>, tx_interval: u64) {
    let mut interval = IntervalStream::new(interval(Duration::from_millis(tx_interval)));
    while let Some(_interval) = interval.next().await {
        pool.write().broadcast();
    }
}

/// Processes transactions directly submitted by client.
pub(crate) async fn process_client_submission<V>(
    smp: Shared<V>,
    transaction: SignedTransaction,
    callback: oneshot::Sender<Result<SubmissionStatus>>,
) where
    V: Validation,
{
    let statuses = process_incoming(&smp, vec![transaction], TxState::NotReady);
    if let Some(status) = statuses.get(0) {
        if callback.send(Ok(status.1.clone())).is_err() {}
    }
}

/// Submits a list of SignedTransaction to the local pool
/// and returns a vector containing AdmissionControlStatus.
pub(crate) fn process_incoming<V>(
    smp: &Shared<V>,
    transactions: Vec<SignedTransaction>,
    state: TxState,
) -> Vec<SubmissionStatusBundle>
where
    V: Validation,
{
    let mut statuses = vec![];

    // Track latency: fetching seq number
    let nonce_and_banace_vec = transactions
        .par_iter()
        .map(|t| {
            get_account_nonce_banace(t.sender()).map_err(|e| {
                error!("txpool: get state error ({})", e);
                e
            })
        })
        .collect::<Vec<_>>();

    let transactions: Vec<_> = transactions
        .into_iter()
        .enumerate()
        .filter_map(|(idx, tx)| {
            if let Ok((seq, banace)) = nonce_and_banace_vec[idx] {
                if tx.nonce() > seq {
                    //check balance for limit fee
                    if tx.gas_limit() as u128 * tx.gas_price() > banace {
                        statuses.push((
                            tx,
                            (
                                TxPoolStatus::new(TxPoolStatusCode::ValidationError),
                                Some(TxPoolValidationStatusCode::InsufficientBalanceFee),
                            ),
                        ));
                    } else {
                        return Some((tx, seq));
                    }
                } else {
                    statuses.push((
                        tx,
                        (
                            TxPoolStatus::new(TxPoolStatusCode::ValidationError),
                            Some(TxPoolValidationStatusCode::SeqTooOld),
                        ),
                    ));
                }
            } else {
                // Failed to get transaction
                statuses.push((
                    tx,
                    (
                        TxPoolStatus::new(TxPoolStatusCode::ValidationError),
                        Some(TxPoolValidationStatusCode::ResourceDoesNotExist),
                    ),
                ));
            }
            None
        })
        .collect();

    // Track latency: VM validation
    let validation_results = transactions
        .par_iter()
        .map(|t| smp.validator.read().validate(&t.0))
        .collect::<Vec<_>>();

    {
        let mut pool = smp.pool.write();
        for (idx, (transaction, seq)) in transactions.into_iter().enumerate() {
            if let Ok(validation_result) = &validation_results[idx] {
                match validation_result.status() {
                    None => {
                        let pool_status = pool.add(transaction.clone(), seq, state);
                        statuses.push((transaction, (pool_status, None)));
                    }
                    Some(validation_status) => {
                        statuses.push((
                            transaction,
                            (
                                TxPoolStatus::new(TxPoolStatusCode::ValidationError),
                                Some(validation_status),
                            ),
                        ));
                    }
                }
            }
        }
    }

    statuses
}

/// Processes transactions from other nodes.
pub(crate) async fn process_broadcast<V>(smp: Shared<V>, transactions: Vec<SignedTransaction>)
where
    V: Validation,
{
    let _results = process_incoming(&smp, transactions, TxState::NotReady);
}

/// Remove transactions that are committed (or rejected) so that we can stop broadcasting them.
pub(crate) async fn process_committed<V>(smp: Shared<V>, msg: TxPoolCommitNotification)
where
    V: Validation,
{
    let tx_size = msg.transactions.len();
    let pool = &smp.pool;
    let start = Instant::now();
    msg.transactions
        .par_iter()
        .for_each(|(sender, transaction)| {
            pool.write().remove(sender, transaction.max_seq);
        });
}

pub fn start_txpool_service(
    config: &configure::TxPoolConfig,
    client_events: ClientReceiver,
    network: PeerNetwork,
) -> (Runtime, BroadCastTxSender, CommitNotificationSender) {
    let runtime = Builder::new_multi_thread()
        .thread_name("shared-mem")
        .enable_all()
        .build()
        .expect("[pool] failed to create runtime");

    let (broadcast_tx_sender, broadcast_tx_events) = futures::channel::mpsc::unbounded();
    let (consensus_committed_sender, committed_events) = futures::channel::mpsc::channel(1024);
    let pool = TX_POOL_INSTANCE_REF.clone();
    {
        pool.write().reinit(config, network);
    }
    let validator = Arc::new(RwLock::new(Validator::new()));
    let executor = runtime.handle();

    let smp = Shared {
        pool: pool.clone(),
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
        pool.clone(),
        config.system_transaction_gc_interval_ms,
    ));
    executor.spawn(broadcast(
        pool.clone(),
        config.broadcast_transaction_interval_ms,
    ));

    (runtime, broadcast_tx_sender, consensus_committed_sender)
}
