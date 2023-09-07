//! Processes that are directly spawned by shared mempool runtime initialization

use crate::{
    CoreMempool,
        status::Status,
        tasks,
        tx_validator::{DiscardedVMStatus, TransactionValidation},
        types::{
            MempoolBroadCastTxReceiver, MempoolClientReceiver, MempoolCommitNotification,
            MempoolCommitNotificationReceiver, MempoolConsensusReceiver, SharedMempool,
        },
};
use anyhow::Result;
use futures::future::{Future, FutureExt};
use futures::{
    channel::{mpsc, oneshot},
    stream::{select_all, FuturesUnordered},
    StreamExt,
};
use parking_lot::RwLock;
use types::TransactionSignRaw;
use std::{
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio::{runtime::Handle, time::interval};
use tokio_stream::wrappers::IntervalStream;

use utils::timing::Timestamp;

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
    mut smp: SharedMempool<V>,
    executor: Handle,
    mut client_events: MempoolClientReceiver,
    mut broadcast_tx_events: MempoolBroadCastTxReceiver,
    mut consensus_requests: MempoolConsensusReceiver,
    mut committed_events: MempoolCommitNotificationReceiver,
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
                bounded_executor.spawn(tasks::process_client_transaction_submission(smp.clone(),transaction,callback)).await;
            },

            transactions = broadcast_tx_events.select_next_some() => {
                // handle_broadcast_event(&mut smp, &bounded_executor, transactions).await;
                bounded_executor.spawn(tasks::process_transaction_broadcast(smp.clone(),transactions)).await;
            },

            request = consensus_requests.select_next_some() => {
                tasks::process_consensus_request(&mut smp, request);
            },

            msg = committed_events.select_next_some()=>{
                bounded_executor.spawn(tasks::process_committed_transactions(smp.clone(),msg, 0,false)).await;
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
pub(crate) async fn broadcast_transaction(
    mempool: Arc<RwLock<CoreMempool>>,
    broadcast_transaction_interval_ms: u64,
) {
    let mut interval = IntervalStream::new(interval(Duration::from_millis(
        broadcast_transaction_interval_ms,
    )));
    while let Some(_interval) = interval.next().await {
        mempool.write().broadcast_transaction();
    }
}
