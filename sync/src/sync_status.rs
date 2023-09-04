
use std::{collections::HashSet, ops::{Deref, DerefMut}};

use network::Endpoint;
use protos::ledger::LedgerHeader;

use fxhash::FxHashMap;
use parking_lot::RwLock;



pub enum SyncState{
    Prepare,
    Synchronizing{
        block_id: u64,
    },
    Synchronized,
}

impl SyncState {
    pub fn is_prepare(&self) -> bool {
        matches!(self, SyncState::Prepare)
    }

    pub fn is_syncing(&self) -> bool {
        matches!(self, SyncState::Synchronizing { .. })
    }

    pub fn is_synced(&self) -> bool {
        matches!(self, SyncState::Synchronized)
    }
}

pub struct ChainStatus{
    pub height: u64,
    pub hash: Vec<u8>,
    pub chain_id: ::std::string::String,
}


pub struct SyncStatus {
    pub status: LedgerHeader,
    pub state: SyncState,
}

impl Default for SyncStatus{
    fn default() -> Self {
        Self { status: LedgerHeader::default(), state: SyncState::Prepare }
    }
}

impl SyncStatus{
    pub fn new(status: LedgerHeader) -> Self{
        Self { status, state: SyncState::Prepare }
    }

    pub fn sync_done(&mut self){
        self.state = SyncState::Synchronized;
    }

    pub fn sync_prepare(&mut self){
        self.state = SyncState::Prepare;
    }

    pub fn sync_ing(&mut self, block_id:u64){
        self.state = SyncState::Synchronizing { block_id };
    }

    pub fn get_height(&self)->u64{
        self.status.get_height()
    }

    pub fn get_hash(&self)->&[u8]{
        self.status.get_hash()
    }

    pub fn is_prepare(&self) -> bool {
        self.state.is_prepare()
    }

    pub fn is_syncing(&self) -> bool {
        self.state.is_syncing()
    }

    pub fn is_synced(&self) -> bool {
        self.state.is_synced()
    }
}

const MAX_SCORE: i64 = 100;
const MIN_SCORE: i64 = 0;
const STARTING_SCORE: i64 = 50;
const SUCCESSFUL_RESPONSE_DELTA: i64 = 1;
const ERROR_RESPONSE_DELTA: i64 = 25;
const IGNORE_PEER_THRESHOLD: i64 = 30;

pub struct PeerInfo{
    block_id: u64,
    score: i64,
}

impl PeerInfo{
    pub fn new(block_id: u64) -> Self{
        Self{
            block_id,
            score:STARTING_SCORE,
        }
    }

    pub fn block_id(&self)->u64{
        self.block_id
    }

    pub fn score(&self)->i64{
        self.score
    }

    pub fn set_block(&mut self, block_id:u64) {
        self.block_id = block_id;
    }

    pub fn update_score_success(&mut self) {
        self.score = i64::min(self.score + SUCCESSFUL_RESPONSE_DELTA, MAX_SCORE);
    }

    pub fn update_score_error(&mut self) {
        self.score = i64::max(self.score - ERROR_RESPONSE_DELTA, IGNORE_PEER_THRESHOLD);
    }

    fn ignored(&self) -> bool{
        self.score.le(&IGNORE_PEER_THRESHOLD)
    }

}

#[derive(Default)]
pub struct Peers(FxHashMap<Endpoint, PeerInfo>);

impl Deref for Peers{
    type Target = FxHashMap<Endpoint, PeerInfo>;
    fn deref(&self) -> &Self::Target{
        &self.0
    }
}

impl DerefMut for Peers{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Peers{
    pub fn select_peer(&self)->Option<(&Endpoint,&PeerInfo)>{
        self.iter().max_by(|x, y| {
            if x.1.block_id == y.1.block_id{
                x.1.score.cmp(&y.1.score)
            }else{
                x.1.block_id.cmp(&y.1.block_id)
            }
        })
    }

    pub fn insert_peer(&mut self, peer_id:Endpoint, block_id:u64) {
        self.entry(peer_id).or_insert(PeerInfo::new(block_id));
    }

    pub fn update_score_success(&mut self, peer_id:Endpoint){
        self.entry(peer_id).and_modify(|pi| pi.update_score_success());
    }

    pub fn update_score_error(&mut self, peer_id:Endpoint){
        self.entry(peer_id).and_modify(|pi| pi.update_score_error());
    }

    pub fn remmove_ignored(&mut self){
        self.retain(|i,pi| !pi.ignored());
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn test_peerinfo(){
        assert_eq!(1,1);
    }

    #[test]
    fn test_peers(){
        assert!(true);
    }
}