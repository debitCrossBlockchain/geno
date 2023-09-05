/// Trait that is implemented by a DB that supports certain public (to client) read APIs
///
///
///
use anyhow::Result;
pub trait DbReader: Send + Sync {
    fn get_sequences(&self, _version: u64) -> Result<u64>;
    fn get_balance(&self, _version: u64) -> Result<u64>;
    fn get_identity(&self, _version: u64,id: bool) -> bool;
}
