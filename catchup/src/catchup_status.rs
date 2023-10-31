
use std::{collections::HashSet, ops::{Deref, DerefMut}};

use network::Endpoint;
use protos::ledger::LedgerHeader;

use fxhash::FxHashMap;
use parking_lot::RwLock;



pub enum CatchupState{
    Prepare,
    Catchuphronized,
    Catchuphronizing{
        block_id: u64,
    },
}

impl CatchupState {
    pub fn is_prepare(&self) -> bool {
        matches!(self, CatchupState::Prepare)
    }

    pub fn is_catchuping(&self) -> bool {
        matches!(self, CatchupState::Catchuphronizing { .. })
    }

    pub fn is_catchuped(&self) -> bool {
        matches!(self, CatchupState::Catchuphronized)
    }
}

#[derive(Default)]
pub struct SyncTimeOut{
    pub height: u64,
    pub ttl: u64,
}

impl SyncTimeOut{
    fn check(&mut self, height: u64) -> bool{
        if self.height == height{
            if self.ttl + 1 > 3{
                self.ttl = 0;
                return true
            }else{
                self.ttl = self.ttl + 1;
            }
        }else{
            self.height = height;
        }

        return false
    }
}


pub struct CatchupStatus {
    pub status: LedgerHeader,
    pub state: CatchupState,
    pub timeout: SyncTimeOut,
}

impl Default for CatchupStatus{
    fn default() -> Self {
        Self { status: LedgerHeader::default(), state: CatchupState::Prepare, timeout: SyncTimeOut::default()}
    }
}

impl CatchupStatus{
    pub fn new(status: LedgerHeader) -> Self{
        Self { status, state: CatchupState::Prepare, timeout: SyncTimeOut{ height: 0, ttl: 0 } }
    }

    pub fn catchup_done(&mut self){
        self.state = CatchupState::Catchuphronized;
    }

    pub fn catchup_prepare(&mut self){
        self.state = CatchupState::Prepare;
    }

    pub fn catchup_ing(&mut self, block_id:u64){
        self.state = CatchupState::Catchuphronizing { block_id };
    }

    pub fn update_status(&mut self, status: LedgerHeader){
        self.status = status;
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

    pub fn is_catchuping(&self) -> bool {
        self.state.is_catchuping()
    }

    pub fn is_catchuped(&self) -> bool {
        self.state.is_catchuped()
    }

    pub fn check(&mut self, height: u64) -> bool{
        self.timeout.check(height)
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