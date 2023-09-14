use fxhash::FxHashMap;
use protobuf::Message;
use protos::consensus::{BftPreparedSet, BftSign, BftViewChange};

use crate::{bft_state::BftState, new_bft_message::NewBftMessage};

use super::{PBFT_NEWVIEW_SEND_INTERVAL, PBFT_VCINSTANCE_TIMEOUT};

pub type BftSignVec = Vec<BftSign>;
pub type BftViewChangeMap = FxHashMap<i64, BftViewChange>;

#[derive(Clone)]
pub struct BftVcInstance {
    pub view_change_msg: BftSign,
    pub view_number: i64,
    pub sequence: u64,
    pub view_changes: BftViewChangeMap,
    pub pre_prepared_env_set: BftPreparedSet,
    // Last prepared related pre-prepared env
    pub view_change_round: i64,
    pub start_time: i64,
    pub last_propose_time: i64,
    pub end_time: i64,
    pub last_newview_time: i64,
    pub msg_buf: BftSignVec,
    //View change message
    pub new_view: BftSign,
    pub new_view_round: u64,
}

impl BftVcInstance {
    pub fn new() -> Self {
        let now = chrono::Local::now().timestamp_millis();
        let self_clone = Self {
            view_change_msg: BftSign::new(),
            view_number: 0,
            sequence: 0,
            view_changes: BftViewChangeMap::default(),
            pre_prepared_env_set: BftPreparedSet::new(),
            view_change_round: 0,
            start_time: now,
            last_propose_time: now,
            end_time: 0,
            last_newview_time: 0,
            msg_buf: BftSignVec::default(),
            new_view: BftSign::new(),
            new_view_round: 1,
        };
        self_clone
    }

    pub fn set_view_number(&mut self, view_number: i64) {
        self.view_number = view_number;
    }

    pub fn set_sequence(&mut self, sequence: u64) {
        self.sequence = sequence;
    }

    pub fn need_send_again(&mut self, current_time: i64) -> bool {
        return ((current_time - self.last_propose_time) > PBFT_VCINSTANCE_TIMEOUT)
            && (self.end_time == 0);
    }

    pub fn need_send_new_view_again(&mut self, current_time: i64) -> bool {
        ((current_time - self.last_newview_time) > PBFT_NEWVIEW_SEND_INTERVAL)
            && self.new_view.is_initialized()
            && (self.end_time > 0)
    }

    pub fn set_last_propose_time(&mut self, current_time: i64) {
        self.last_propose_time = current_time;
    }

    pub fn change_complete(&mut self, current_time: i64) {
        self.end_time = current_time;
    }

    pub fn set_last_newview_time(&mut self, current_time: i64) {
        self.last_newview_time = current_time;
    }

    pub fn send_new_view_again(&mut self, state: &BftState, current_time: i64) {
        self.new_view_round += 1;
        let msg = NewBftMessage::inc_message_round(&state, &self.new_view, self.new_view_round);
        self.set_last_newview_time(current_time);
        state.broadcast_message(&msg);
    }

    pub fn send_new_view(&mut self, state: &mut BftState, bft_sign: BftSign, current_time: i64) {
        self.set_last_newview_time(current_time);
        self.new_view.clone_from(&bft_sign);
        state.broadcast_message(&bft_sign);
    }

    pub fn should_terminated(&self, current_time: i64, time_out: i64) -> bool {
        current_time - self.start_time >= time_out
    }

    pub fn values(&self) -> Vec<BftViewChange> {
        let mut vecs: Vec<BftViewChange> = Vec::new();
        for item in self.view_changes.values() {
            vecs.push(item.clone());
        }
        vecs
    }

    pub fn clone_view_changes(&mut self, view_changes: &[BftViewChange]) {
        for view_change in view_changes {
            self.view_changes
                .insert(view_change.get_base().get_replica_id(), view_change.clone());
        }
    }
}
