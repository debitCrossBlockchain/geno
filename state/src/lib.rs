pub mod account_frame;
pub mod cache_state;
// pub mod memory_statedb;
// pub mod reading_trie;
pub mod trie;
pub mod trie_hashdb;

pub use account_frame::AccountFrame;
pub use cache_state::CacheState;
// pub use memory_statedb::MemoryStateDB;
pub use trie::{TrieHash, TrieReader, TrieWriter};

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
pub use trie_hashdb::TrieHashDB;

// pub use crate::reading_trie::{
//     reading_trie_get, reading_trie_get_nonce_banace, reading_trie_update_account_cache, ReadingTrie,
// };
// use once_cell::sync::Lazy;

// pub static READING_TRIE: Lazy<Mutex<ReadingTrie>> =
//     Lazy::new(|| Mutex::new(ReadingTrie::default()));

// #[macro_use]
// extern crate lazy_static;
// lazy_static! {
//     pub static ref ReadingTrieRef: Arc<RwLock<ReadingTrie>> =
//         Arc::new(RwLock::new(ReadingTrie::default()));
// }
