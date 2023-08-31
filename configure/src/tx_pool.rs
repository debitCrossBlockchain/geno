use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct TxPoolConfig {
    pub capacity: usize,
    pub capacity_per_user: usize,
    // number of failovers to broadcast to when the primary network is alive
    pub default_failovers: usize,
    pub max_broadcasts_per_peer: usize,
    pub mempool_snapshot_interval_secs: u64,
    pub shared_mempool_ack_timeout_ms: u64,
    pub shared_mempool_backoff_interval_ms: u64,
    pub shared_mempool_batch_size: usize,
    pub shared_mempool_max_concurrent_inbound_syncs: usize,
    pub shared_mempool_tick_interval_ms: u64,
    pub system_transaction_timeout_secs: u64,
    pub system_transaction_gc_interval_ms: u64,
    pub broadcast_max_batch_size: usize,
    pub broadcast_transaction_interval_ms: u64,
}

impl Clone for TxPoolConfig {
    fn clone(&self) -> Self {
        Self {
            capacity: self.capacity,
            capacity_per_user: self.capacity_per_user,
            default_failovers: self.default_failovers,
            max_broadcasts_per_peer: self.max_broadcasts_per_peer,
            mempool_snapshot_interval_secs: self.mempool_snapshot_interval_secs,
            shared_mempool_ack_timeout_ms: self.shared_mempool_ack_timeout_ms,
            shared_mempool_backoff_interval_ms: self.shared_mempool_backoff_interval_ms,
            shared_mempool_batch_size: self.shared_mempool_batch_size,
            shared_mempool_max_concurrent_inbound_syncs: self
                .shared_mempool_max_concurrent_inbound_syncs,
            shared_mempool_tick_interval_ms: self.shared_mempool_tick_interval_ms,
            system_transaction_timeout_secs: self.system_transaction_timeout_secs,
            system_transaction_gc_interval_ms: self.system_transaction_gc_interval_ms,
            broadcast_max_batch_size: self.broadcast_max_batch_size,
            broadcast_transaction_interval_ms: self.broadcast_transaction_interval_ms,
        }
    }
}

impl Default for TxPoolConfig {
    fn default() -> TxPoolConfig {
        TxPoolConfig {
            shared_mempool_tick_interval_ms: 50,
            shared_mempool_backoff_interval_ms: 30_000,
            shared_mempool_batch_size: 100000,
            shared_mempool_ack_timeout_ms: 2_000,
            shared_mempool_max_concurrent_inbound_syncs: 2,
            max_broadcasts_per_peer: 1,
            mempool_snapshot_interval_secs: 180,
            capacity: 1_000_000,
            capacity_per_user: 10000,
            default_failovers: 3,
            system_transaction_timeout_secs: 600,
            system_transaction_gc_interval_ms: 60_000,
            broadcast_max_batch_size: 100,
            broadcast_transaction_interval_ms: 500,
        }
    }
}
