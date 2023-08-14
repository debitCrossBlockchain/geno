use fxhash::FxHashMap;
use parking_lot::RwLock;
use std::collections::HashSet;
use std::net::SocketAddr;

pub struct BroadcastRecord {
    pub time_stamp: i64,
    pub peers: HashSet<SocketAddr>,
}

impl Default for BroadcastRecord {
    fn default() -> Self {
        Self {
            time_stamp: chrono::Local::now().timestamp(),
            peers: HashSet::default(),
        }
    }
}

impl Clone for BroadcastRecord {
    fn clone(&self) -> Self {
        Self {
            time_stamp: self.time_stamp.clone(),
            peers: self.peers.clone(),
        }
    }
}

#[derive(Default)]
pub struct KnownBroadcasts(RwLock<FxHashMap<String, BroadcastRecord>>);

impl KnownBroadcasts {
    pub fn add_one(&self, hash: &str, peer_id: SocketAddr) -> bool {
        if let Some(record) = self.0.write().get_mut(hash) {
            record.peers.insert(peer_id);
            return false;
        }
        let mut record = BroadcastRecord::default();
        record.peers.insert(peer_id);
        self.insert(hash.to_string(), record);
        return true;
    }

    pub fn add_set(&self, hash: &str, peer_ids: HashSet<SocketAddr>) -> bool {
        if let Some(record) = self.0.write().get_mut(hash) {
            record.peers.extend(peer_ids);
            return false;
        }
        let mut record = BroadcastRecord::default();
        record.peers.extend(peer_ids);
        self.insert(hash.to_string(), record);
        return true;
    }

    pub fn insert(&self, hash: String, record: BroadcastRecord) {
        self.0.write().insert(hash, record);
    }

    pub fn get_peers(&self, hash: &str) -> HashSet<SocketAddr> {
        if let Some(record) = self.0.read().get(hash) {
            return record.peers.clone();
        }
        HashSet::default()
    }

    pub fn remove(&self, hash: &str) -> bool {
        self.0.write().remove(hash).is_some()
    }

    pub fn contains(&self, hash: &str) -> bool {
        self.0.read().contains_key(hash)
    }

    pub fn has(&self, hash: &str) -> Option<HashSet<SocketAddr>> {
        if let Some(record) = self.0.read().get(hash) {
            return Some(record.peers.clone());
        }
        None
    }

    pub fn check_timeout(&self) {
        let current_time = chrono::Local::now().timestamp();

        self.0
            .write()
            .retain(|k, v| (current_time - v.time_stamp) <= 60);
    }
}
