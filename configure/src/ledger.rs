use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct Ledger {
    pub commit_interval: i64,
    // pub hardfork_points: Vec<String>,
}

impl Clone for Ledger {
    fn clone(&self) -> Self {
        Self {
            commit_interval: self.commit_interval,
            // hardfork_points: self.hardfork_points.clone(),
        }
    }
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            commit_interval: 10,
            // hardfork_points: Vec::default(),
        }
    }
}
