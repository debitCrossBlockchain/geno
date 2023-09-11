use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct TxPoolConfig {
    pub capacity: usize,
    pub capacity_per_user: usize,
    // number of failovers to broadcast to when the primary network is alive
    pub default_failovers: usize,
    pub max_broadcasts_per_peer: usize,
    pub snapshot_interval_secs: u64,
    pub ack_timeout_ms: u64,
    pub backoff_interval_ms: u64,
    pub batch_size: usize,
    pub max_concurrent_inbound_syncs: usize,
    pub tick_interval_ms: u64,
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
            snapshot_interval_secs: self.snapshot_interval_secs,
            ack_timeout_ms: self.ack_timeout_ms,
            backoff_interval_ms: self.backoff_interval_ms,
            batch_size: self.batch_size,
            max_concurrent_inbound_syncs: self
                .max_concurrent_inbound_syncs,
            tick_interval_ms: self.tick_interval_ms,
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
            tick_interval_ms: 50,
            backoff_interval_ms: 30_000,
            batch_size: 100000,
            ack_timeout_ms: 2_000,
            max_concurrent_inbound_syncs: 2,
            max_broadcasts_per_peer: 1,
            snapshot_interval_secs: 180,
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
