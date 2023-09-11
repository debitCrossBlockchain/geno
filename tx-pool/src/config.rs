
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
    pub tx_timeout_secs: u64,
    pub tx_gc_interval_ms: u64,
}

impl Default for TxPoolConfig {
    fn default() -> TxPoolConfig {
        TxPoolConfig {
            tick_interval_ms: 50,
            backoff_interval_ms: 30_000,
            batch_size: 100,
            ack_timeout_ms: 2_000,
            max_concurrent_inbound_syncs: 2,
            max_broadcasts_per_peer: 1,
            snapshot_interval_secs: 180,
            capacity: 1_000_000,
            capacity_per_user: 100,
            default_failovers: 3,
            tx_timeout_secs: 600,
            tx_gc_interval_ms: 60_000,
        }
    }
}
