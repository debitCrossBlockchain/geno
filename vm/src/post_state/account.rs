use derive_more::Deref;
use revm::primitives::{B160, B256, KECCAK_EMPTY, U256};
use std::collections::{btree_map::Entry, BTreeMap};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PostAccount {
    /// Account nonce.
    pub nonce: u64,
    /// Account balance.
    pub balance: U256,
    /// Hash of the account's bytecode.
    pub bytecode_hash: Option<B256>,
}

impl PostAccount {
    /// Whether the account has bytecode.
    pub fn has_bytecode(&self) -> bool {
        self.bytecode_hash.is_some()
    }

    /// After SpuriousDragon empty account is defined as account with nonce == 0 && balance == 0 &&
    /// bytecode = None.
    pub fn is_empty(&self) -> bool {
        let is_bytecode_empty = match self.bytecode_hash {
            None => true,
            Some(hash) => hash == KECCAK_EMPTY,
        };

        self.nonce == 0 && self.balance == U256::ZERO && is_bytecode_empty
    }

    /// Returns an account bytecode's hash.
    /// In case of no bytecode, returns [`KECCAK_EMPTY`].
    pub fn get_bytecode_hash(&self) -> B256 {
        match self.bytecode_hash {
            Some(hash) => hash,
            None => KECCAK_EMPTY,
        }
    }
}

#[derive(Default, Clone, Eq, PartialEq, Debug, Deref)]
pub struct AccountChanges {
    /// The inner mapping of block changes.
    #[deref]
    pub inner: BTreeMap<u64, BTreeMap<B160, Option<PostAccount>>>,
    /// Hand tracked change size.
    pub size: usize,
}

impl AccountChanges {
    /// Insert account change at specified block number. The value is **not** updated if it already
    /// exists.
    pub fn insert(
        &mut self,
        block: u64,
        address: B160,
        old: Option<PostAccount>,
        new: Option<PostAccount>,
    ) {
        match self.inner.entry(block).or_default().entry(address) {
            Entry::Vacant(entry) => {
                self.size += 1;
                entry.insert(old);
            }
            Entry::Occupied(entry) => {
                // If the account state is the same before and after this block, collapse the state
                // changes.
                if entry.get() == &new {
                    entry.remove();
                    self.size -= 1;
                }
            }
        }
    }

    /// Insert account changes at specified block number. The values are **not** updated if they
    /// already exist.
    pub fn insert_for_block(&mut self, block: u64, changes: BTreeMap<B160, Option<PostAccount>>) {
        let block_entry = self.inner.entry(block).or_default();
        for (address, account) in changes {
            if let Entry::Vacant(entry) = block_entry.entry(address) {
                entry.insert(account);
                self.size += 1;
            }
        }
    }

    /// Drain and return any entries above the target block number.
    pub fn drain_above(
        &mut self,
        target_block: u64,
    ) -> BTreeMap<u64, BTreeMap<B160, Option<PostAccount>>> {
        let mut evicted = BTreeMap::new();
        self.inner.retain(|block_number, accounts| {
            if *block_number > target_block {
                self.size -= accounts.len();
                evicted.insert(*block_number, accounts.clone());
                false
            } else {
                true
            }
        });
        evicted
    }

    /// Retain entries only above specified block number.
    pub fn retain_above(&mut self, target_block: u64) {
        self.inner.retain(|block_number, accounts| {
            if *block_number > target_block {
                true
            } else {
                self.size -= accounts.len();
                false
            }
        });
    }
}
