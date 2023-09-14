use fxhash::FxHashMap;

use crate::{
    bft_check_value::CheckValueResult,
    bft_state::{BftInstancePhase, BftState},
    handler::handler_instance::HandlerInstance,
    instance::{
        bft_instance::BftInstance, bft_instance_index::BftInstanceIndex,
        bft_vc_instance::BftVcInstance,
    },
    new_bft_message::NewBftMessage,
};
use itertools::Itertools;
use protos::consensus::BftSign;
use tracing::{error, info};

pub type AbnormalRecords = FxHashMap<String, i64>;
pub type BftInstanceMap = FxHashMap<BftInstanceIndex, BftInstance>;
pub type BftVcInstanceMap = FxHashMap<i64, BftVcInstance>;

pub struct BftLog {
    pub(crate) abnormal_records: AbnormalRecords,
    //For synchronization,catch up
    pub(crate) catch_up_instances: BftInstanceMap,
    //For bft instance
    pub(crate) instances: BftInstanceMap,
    //For view change
    pub(crate) vc_instances: BftVcInstanceMap,
}

impl Default for BftLog {
    fn default() -> Self {
        Self {
            abnormal_records: AbnormalRecords::default(),
            catch_up_instances: BftInstanceMap::default(),
            instances: BftInstanceMap::default(),
            vc_instances: BftVcInstanceMap::default(),
        }
    }
}

impl BftLog {
    pub fn instances(&self) -> FxHashMap<BftInstanceIndex, BftInstance> {
        let mut iters: FxHashMap<BftInstanceIndex, BftInstance> = FxHashMap::default();
        for iter in self.instances.iter() {
            iters.insert(iter.0.clone(), iter.1.clone());
        }
        iters
    }

    pub fn check_instances_timeout(&mut self, state: &mut BftState, current_time: i64) {
        if !state.is_validator() {
            return;
        }

        let mut last_index = BftInstanceIndex::new(0, 0);
        let instances_clone = self.instances.clone();
        let keys = instances_clone.keys();
        for index in keys.sorted_by(|&a, &b| BftInstanceIndex::cmp(a, b)) {
            let (view_number, sequence) = (index.view_number, index.sequence);
            if let Some(instance) = self.instances.get_mut(index) {
                //Check if it times out
                //if instance.is_expire(current_time) && (!instance.have_send_view_change) {
                if instance.is_expire(current_time) {
                    info!(parent:state.span(),
                        "check_instances_timeout bft instance timeout, view_number({}), sequence({}), phase({:?}) have_send_view_change({})",
                        view_number, sequence, instance.phase,instance.have_send_view_change
                    );
                    if !instance.have_send_view_change {
                        state.start_view_change(&instances_clone);
                        instance.have_send_view_change = true;
                    }
                }

                //Check if we should send the pre-prepare again
                if instance.need_send_pre_prepare_again(current_time)
                    && state.view_active
                    && instance.pre_prepare_msg.has_bft()
                {
                    info!(parent:state.span(),"check_instances_timeout Send pre-prepare message again actively: view_number({}), sequence({}), round number({})", view_number, sequence, instance.pre_prepare_round);
                    instance.send_pre_prepare_again(state, current_time);
                }

                if (instance.phase as i64) >= (BftInstancePhase::PREPARED as i64) {
                    last_index.clone_from(index);
                }

                //for keep same with instances
                // instances_clone.insert(index.clone(), instance.clone());
            }
        }

        if let Some(last_instance) = self.instances.get_mut(&last_index) {
            if last_instance.check_value == CheckValueResult::Valid
                && last_instance.need_send_commit_again(current_time)
            {
                //For broadcast only
                match last_instance.prepares_begin() {
                    Some(prepare) => {
                        last_instance.commit_round += 1;
                        let commit =
                            NewBftMessage::new_commit(state, &prepare, last_instance.commit_round);

                        info!(parent:state.span(),"check_instances_timeout Send commit message again actively: view_number({}), sequence({}), round number({})", last_index.view_number, last_index.sequence, last_instance.commit_round);
                        state.broadcast_message(&commit);
                        last_instance.set_last_commit_send_time(current_time);
                    }
                    None => {
                        error!(parent:state.span(),"check_instances_timeout Can not find prepare from prepares of instance");
                    }
                }
            }
        }
    }

    pub fn check_vc_instances_timeout(&mut self, state: &mut BftState, current_time: i64) {
        if !state.is_validator() {
            return;
        }
        //Check if the 'view change' times out, and get the last 'new view' just sent.
        let mut last_view_number: i64 = 0;
        let vc_instances_clone = self.vc_instances.clone();
        let keys = vc_instances_clone.keys();
        for view_number in keys.sorted() {
            if let Some(vc_instance) = self.vc_instances.get_mut(view_number) {
                if vc_instance.need_send_again(current_time)
                    && vc_instance.view_change_msg.has_bft()
                {
                    vc_instance.new_view_round += 1;
                    let msg = NewBftMessage::inc_message_round(
                        &state,
                        &vc_instance.view_change_msg,
                        vc_instance.new_view_round,
                    );
                    info!(parent:state.span(),
                        "check_vc_instances_timeout Send view-change message again actively: view_number({}), round number({})",
                        view_number, vc_instance.view_change_round
                    );
                    state.broadcast_message(&msg);
                    vc_instance.set_last_propose_time(current_time);
                }

                if vc_instance.need_send_new_view_again(current_time)
                    && ((vc_instance.view_number % (state.validators.len() as i64))
                        == state.replica_id)
                {
                    //last vc_instance
                    last_view_number = view_number.clone();
                }
            }
        }

        if let Some(last_vc_instance) = self.vc_instances.get_mut(&last_view_number) {
            info!(parent:state.span(),
                "check_vc_instances_timeout Send new view message again actively: view_number({}), round number({})",
                last_vc_instance.view_number, last_vc_instance.new_view_round
            );
            last_vc_instance.send_new_view_again(state, current_time);
        }
    }

    pub fn get_instance(&mut self, index: &BftInstanceIndex) -> Option<BftInstance> {
        if let Some(value) = self.instances.get(index) {
            Some(value.clone());
        }
        None
    }

    //handle process
    pub fn initialize_instance(&mut self, bft_sign: &BftSign, state: &BftState) {
        let index = BftInstanceIndex::index(bft_sign);
        let is_exist = self.instances.contains_key(&index);
        let (view_number, sequence) = (index.view_number, index.sequence);
        if !is_exist {
            info!(parent:state.span(),
                "Create bft instance(view_number:{}, sequence:{}) type({:?})",
                view_number,
                sequence,
                bft_sign.get_bft().get_msg_type()
            );
            self.instances.insert(index, BftInstance::new());
            //Delete the seq which is not the same view
            self.instances
                .retain(|key, _| !((key.view_number < view_number) && (key.sequence == sequence)));
        }
        let phase = &BftInstancePhase::as_phase(&bft_sign.get_bft().get_msg_type());
        if let Some(instance) = self.instances.get_mut(&index) {
            instance.update_msg_buf(phase, bft_sign);
        }
    }

    pub fn handle_instance(
        &mut self,
        bft_sign: &BftSign,
        state: &mut BftState,
        check_value: CheckValueResult,
        trigger_committed: &mut bool,
    ) -> bool {
        let index = BftInstanceIndex::index(bft_sign);
        if let Some(instance) = self.instances.get_mut(&index) {
            let result = HandlerInstance::handler_primary(
                instance,
                bft_sign,
                state,
                check_value,
                trigger_committed,
            );
            return result;
        }
        error!(parent:state.span(),
            "Can not find instance,index(view_number:{},sequence:{})",
            index.view_number, index.sequence
        );
        false
    }

    ///vc_instance
    pub(crate) fn change_vc_instance_complete(&mut self, view_number: i64) {
        if let Some(vc_instance) = self.vc_instances.get_mut(&view_number) {
            vc_instance.change_complete(chrono::Local::now().timestamp_millis());
        };
    }
}
