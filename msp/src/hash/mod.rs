use crate::hash::sha256::{hash_sha256, verify_sha256};
use crate::hash::sm3::{hash_sm3, verify_sm3};

pub mod sha256;
pub mod sm3;
#[derive(Clone, Copy)]
pub enum HASH_TYPE {
    HASH_TYPE_SHA256,
    SM3,
}

pub struct Hash {
    hash_type: HASH_TYPE,
}

impl Default for Hash {
    fn default() -> Self {
        Self {
            hash_type: HASH_TYPE::HASH_TYPE_SHA256, // hash_type: HASH_TYPE::SM3
        }
    }
}

impl Hash {
    pub const HASH_LENGTH: usize = 32;

    pub fn new(&mut self, in_hash_type: HASH_TYPE) -> Self {
        Self {
            hash_type: in_hash_type,
        }
    }

    pub fn hash(&self, bytes: &[u8]) -> Vec<u8> {
        match self.hash_type {
            HASH_TYPE::HASH_TYPE_SHA256 => hash_sha256(bytes),
            HASH_TYPE::SM3 => hash_sm3(bytes),
        }
    }

    pub fn verify_hash(&self, content: &[u8], content_hash: &[u8]) -> bool {
        match self.hash_type {
            HASH_TYPE::HASH_TYPE_SHA256 => verify_sha256(content, content_hash),
            HASH_TYPE::SM3 => verify_sm3(content, content_hash),
        }
    }

    pub fn zero() -> Vec<u8> {
        let hash = [0; Self::HASH_LENGTH];
        hash.to_vec()
    }
}
