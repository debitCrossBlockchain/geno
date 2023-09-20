pub mod account_frame;
pub mod cache_state;
pub mod reading_trie;
pub mod trie;
pub mod trie_hashdb;

pub use account_frame::AccountFrame;
pub use cache_state::CacheState;
pub use trie::{TrieHash, TrieReader, TrieWriter};

use parking_lot::RwLock;
use std::sync::Arc;
pub use trie_hashdb::TrieHashDB;

use crate::reading_trie::ReadingTrie;

pub const TRIE_KEY_MAX_LEN: usize = 63;

#[macro_use]
extern crate lazy_static;
lazy_static! {
    pub static ref READING_TRIE_REF: Arc<RwLock<ReadingTrie>> =
        Arc::new(RwLock::new(ReadingTrie::default()));
}
