use fxhash::FxHashMap;

use protos::consensus::{BftCommit, BftPrePrepare, BftPrepare, BftSign};

use crate::{
    bft_check_value::CheckValueResult,
    bft_state::{BftInstancePhase, BftState},
    new_bft_message::NewBftMessage,
};

use super::{PBFT_COMMIT_SEND_INTERVAL, PBFT_INSTANCE_TIMEOUT};

pub type BftPrepareMap = FxHashMap<i64, BftPrepare>;
pub type BftCommitMap = FxHashMap<i64, BftCommit>;
pub type BftSignMaps = FxHashMap<BftInstancePhase, Vec<BftSign>>;

pub struct BftInstance {
    pub phase: BftInstancePhase,
    pub phase_item: i64,
    pub pre_prepare: BftPrePrepare,
    pub prepares: BftPrepareMap,
    pub commits: BftCommitMap,
    pub msg_buf: BftSignMaps,
    pub pre_prepare_msg: BftSign,
    pub start_time: i64,
    pub end_time: i64,
    pub last_propose_time: i64,
    pub last_commit_send_time: i64,
    pub have_send_view_change: bool,
    pub pre_prepare_round: u64,
    pub commit_round: u64,
    pub check_value: CheckValueResult,
    pub commit_complete: bool,
}

impl Clone for BftInstance {
    fn clone(&self) -> Self {
        Self {
            phase: self.phase,
            phase_item: self.phase_item,
            pre_prepare: self.pre_prepare.clone(),
            prepares: self.prepares(),
            commits: self.commits(),
            msg_buf: self.msg_buf(),
            pre_prepare_msg: self.pre_prepare_msg.clone(),
            start_time: self.start_time,
            end_time: self.end_time,
            last_propose_time: self.last_propose_time,
            last_commit_send_time: self.last_commit_send_time,
            have_send_view_change: self.have_send_view_change,
            pre_prepare_round: self.pre_prepare_round,
            commit_round: self.commit_round,
            check_value: self.check_value,
            commit_complete: self.commit_complete,
        }
    }
}

impl BftInstance {
    pub fn new() -> Self {
        let now = chrono::Local::now().timestamp_millis();
        let self_clone = Self {
            phase: BftInstancePhase::NONE,
            phase_item: 0,
            pre_prepare: BftPrePrepare::default(),
            prepares: BftPrepareMap::default(),
            commits: BftCommitMap::default(),
            msg_buf: BftSignMaps::default(),
            pre_prepare_msg: BftSign::default(),
            start_time: now,
            end_time: 0,
            last_propose_time: now,
            last_commit_send_time: 0,
            have_send_view_change: false,
            pre_prepare_round: 1,
            commit_round: 1,
            check_value: CheckValueResult::Valid,
            commit_complete: false,
        };
        self_clone
    }

    pub fn prepares(&self) -> BftPrepareMap {
        let mut prepare_map = BftPrepareMap::default();
        for (key, value) in self.prepares.iter() {
            prepare_map.insert(key.clone(), value.clone());
        }
        prepare_map
    }

    pub fn commits(&self) -> BftCommitMap {
        let mut commits_map = BftCommitMap::default();
        for (key, value) in self.commits.iter() {
            commits_map.insert(key.clone(), value.clone());
        }
        commits_map
    }

    pub fn msg_buf(&self) -> BftSignMaps {
        let mut buf = BftSignMaps::default();
        for (key, value) in self.msg_buf.iter() {
            let mut vec: Vec<BftSign> = Vec::new();
            for it in value {
                vec.push(it.clone());
            }
            buf.insert(key.clone(), vec);
        }
        buf
    }

    pub fn prepares_begin(&self) -> Option<BftPrepare> {
        for (_, value) in self.prepares.iter() {
            let mut prepare = BftPrepare::new();
            prepare.clone_from(&value);
            return Some(value.clone());
        }
        None
    }

    pub fn get_bft_sign(&self, phase: &BftInstancePhase, index: usize) -> Option<BftSign> {
        if let Some(bft_sign_vec) = self.msg_buf.get(phase) {
            if let Some(bft_sign_ref) = bft_sign_vec.get(index) {
                let mut bft_sign = BftSign::new();
                bft_sign.clone_from(bft_sign_ref);
                return Some(bft_sign);
            }
        }
        None
    }

    pub fn update_msg_buf(&mut self, phase: &BftInstancePhase, bft_sign: &BftSign) {
        if let Some(bft_sign_vec) = self.msg_buf.get_mut(phase) {
            bft_sign_vec.push(bft_sign.clone());
        } else {
            let mut vec: Vec<BftSign> = Vec::new();
            vec.push(bft_sign.clone());
            self.msg_buf.insert(phase.clone(), vec);
        }
    }

    pub fn get_bft_sign_vec(&self, phase: &BftInstancePhase) -> Vec<BftSign> {
        let mut vec: Vec<BftSign> = Vec::new();
        if let Some(vec_sign) = self.msg_buf.get(phase) {
            vec.clone_from(vec_sign);
            return vec;
        }
        vec
    }

    pub fn is_expire(&self, current_time: i64) -> bool {
        return (current_time - self.start_time > PBFT_INSTANCE_TIMEOUT)
            && ((self.phase.clone() as i64) < (BftInstancePhase::COMMITTED as i64));
    }

    pub fn need_send_pre_prepare_again(&self, current_time: i64) -> bool {
        return (current_time - self.last_propose_time >= PBFT_INSTANCE_TIMEOUT / 4)
            && ((self.phase.clone() as i64) < (BftInstancePhase::COMMITTED as i64));
    }

    pub fn send_pre_prepare_again(&mut self, state: &BftState, current_time: i64) {
        self.pre_prepare_round += 1;
        let msg =
            NewBftMessage::inc_message_round(state, &self.pre_prepare_msg, self.pre_prepare_round);
        state.broadcast_message(&msg);
        self.set_last_propose_time(current_time);
    }

    pub fn need_send_commit_again(&mut self, current_time: i64) -> bool {
        //If the commit message has been sent successfully, then it will be sent regularly later
        return (self.last_commit_send_time != 0)
            && (current_time - self.last_commit_send_time) >= PBFT_COMMIT_SEND_INTERVAL
            && ((self.phase.clone() as i64) >= (BftInstancePhase::PREPARED as i64));
    }

    pub fn set_last_propose_time(&mut self, current_time: i64) {
        self.last_propose_time = current_time;
    }

    pub fn set_last_commit_send_time(&mut self, current_time: i64) {
        self.last_commit_send_time = current_time;
    }

    pub fn is_committing(&self, state: &mut BftState) -> bool {
        return if (self.commits.len() >= (state.quorum_size() + 1))
            && ((self.phase as i64) < (BftInstancePhase::COMMITTED as i64))
        {
            true
        } else {
            false
        };
    }
}
