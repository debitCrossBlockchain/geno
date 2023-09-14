use protos::consensus::*;
use tracing::{error, info};

use crate::{
    bft_consensus::BftConsensus,
    handler::handler_commit::HandlerCommit,
    instance::{bft_instance::BftInstance, bft_instance_index::BftInstanceIndex},
    new_bft_message::NewBftMessage,
};

pub mod handler_commit;
pub mod handler_instance;
pub mod handler_new_view;
pub mod handler_pre_prepare;
pub mod handler_prepare;
pub mod handler_view_change;

#[derive(Clone, Default)]
pub struct BftHandler {}

impl BftHandler {
    pub fn get_seq(bft_sign: &BftSign) -> u64 {
        let bft = bft_sign.get_bft();
        let mut sequence = 0;
        match bft.get_msg_type() {
            BftMessageType::PRE_PREPARE => {
                if bft.has_pre_prepare() {
                    sequence = bft.get_pre_prepare().get_base().get_sequence();
                }
            }
            BftMessageType::PREPARE => {
                if bft.has_prepare() {
                    sequence = bft.get_prepare().get_base().get_sequence();
                }
            }
            BftMessageType::COMMIT => {
                if bft.has_commit() {
                    sequence = bft.get_commit().get_base().get_sequence();
                }
            }
            BftMessageType::VIEW_CHANGE => {
                if bft.has_view_change() {
                    sequence = bft.get_view_change().get_base().get_sequence();
                }
            }
            BftMessageType::NEW_VIEW => {
                if bft.has_new_view() {
                    sequence = bft.get_new_view().get_base().get_sequence();
                }
            }
            _ => {}
        }
        return sequence;
    }

    pub fn bft_value(bft_sign: &BftSign) -> Vec<u8> {
        let bft = bft_sign.get_bft();
        let mut values: Vec<u8> = Vec::new();
        match bft.get_msg_type() {
            BftMessageType::PRE_PREPARE => {
                if bft.has_pre_prepare() {
                    values.clone_from_slice(bft.get_pre_prepare().get_value());
                }
            }
            BftMessageType::VIEW_CHANGE_VALUE => {
                if bft.has_view_change_value() {
                    values.clone_from_slice(
                        bft.get_view_change_value()
                            .get_prepared_set()
                            .get_pre_prepare()
                            .get_bft()
                            .get_pre_prepare()
                            .get_value(),
                    );
                }
            }
            BftMessageType::NEW_VIEW => {
                if bft.has_new_view() {
                    values.clone_from_slice(
                        bft.get_new_view()
                            .get_pre_prepare()
                            .get_bft()
                            .get_pre_prepare()
                            .get_value(),
                    );
                }
            }
            _ => {}
        }
        return values;
    }

    pub fn create_instance(bft_consensus: &mut BftConsensus, bft_sign: &BftSign) -> bool {
        //create instance if not exist
        let bft = bft_sign.get_bft();
        let index = BftInstanceIndex::index(bft_sign);
        let (view_number, sequence) = (index.view_number, index.sequence);
        let same_view = bft_consensus.state.view_number == view_number;
        if !same_view {
            info!(parent:bft_consensus.span(),
                "The message(type:{:?})'s view number({}) != this view number({}), desc({})",
                bft.get_msg_type(),
                view_number,
                bft_consensus.state.view_number,
                NewBftMessage::bft_desc(bft)
            );

            if sequence > bft_consensus.state.last_exe_sequence {
                if bft.get_msg_type() == BftMessageType::COMMIT {
                    BftHandler::catch_up_commit(bft_consensus, bft_sign);
                }
            }
            return false;
        }

        if !bft_consensus.state.view_active {
            info!(parent:bft_consensus.span(),"The message(type:{:?}, sequence {}) would not be processed when view is not active", bft.get_msg_type(),sequence);
            return false;
        }

        if sequence <= bft_consensus.state.last_exe_sequence {
            info!(parent:bft_consensus.span(),
                "bft current sequence({}) <= last sequence({}), then don't create instance.",
                sequence, bft_consensus.state.last_exe_sequence
            );
            return false;
        }
        bft_consensus
            .logs
            .initialize_instance(bft_sign, &bft_consensus.state);
        true
    }

    pub fn catch_up_commit(bft_consensus: &mut BftConsensus, bft_sign: &BftSign) -> bool {
        //Check if it exists in normal object
        let bft = bft_sign.get_bft();
        let commit = bft.get_commit();
        let index = BftInstanceIndex::new(
            commit.get_base().get_view_number(),
            commit.get_base().get_sequence(),
        );
        //Check if it exists in normal object
        if let Some(instance) = bft_consensus.logs.instances.get_mut(&index) {
            let mut trigger_committed = false;
            let result = HandlerCommit::handle_commit(
                &mut bft_consensus.state,
                bft,
                instance,
                &mut trigger_committed,
            );
            info!(parent:bft_consensus.state.span(),"Received trace out but normal commit message from {} round number{})",
                NewBftMessage::base_info_desc(commit.get_base()),bft.get_round_number());

            if !result {
                return result;
            } else {
                if trigger_committed {
                    return bft_consensus.execute_value(bft_sign);
                }
                return result;
            }
        }

        if !bft_consensus.logs.catch_up_instances.contains_key(&index) {
            let mut instance = BftInstance::new();
            instance
                .pre_prepare
                .set_value_digest(Vec::from(commit.get_value_digest()));
            bft_consensus
                .logs
                .catch_up_instances
                .insert(index, instance);
        }

        if let Some(instance) = bft_consensus.logs.catch_up_instances.get(&index) {
            if instance.pre_prepare.get_value_digest() != commit.get_value_digest() {
                error!(parent:bft_consensus.span(),"The commit message({}) is not equal to pre-prepare message",
                NewBftMessage::base_info_desc(commit.get_base()));
                return false;
            }

            info!(parent:bft_consensus.span(),"Received trace out commit message from{}, round number({})",
            NewBftMessage::base_info_desc(commit.get_base()), bft.get_round_number());
            let mut new_instance = BftInstance::new();
            new_instance.clone_from(instance);
            new_instance
                .commits
                .insert(commit.get_base().get_replica_id(), commit.clone());
            bft_consensus
                .logs
                .catch_up_instances
                .insert(index, new_instance.clone());
            BftHandler::handle_catch_up(bft_consensus, &index, &new_instance);
        }
        return true;
    }

    pub fn handle_catch_up(
        bft_consensus: &mut BftConsensus,
        index: &BftInstanceIndex,
        instance: &BftInstance,
    ) {
        if instance.commits.len() >= (bft_consensus.quorum_size() + 1) {
            info!(parent:bft_consensus.span(),
                "commit trace out bft, vn({}), sequence({})",
                index.view_number, index.sequence
            );

            if ((index.sequence - bft_consensus.last_exe_sequence())
                >= bft_consensus.ckp_interval())
                || ((index.view_number - bft_consensus.view_number()) >= 1)
            {
                info!(parent:bft_consensus.span(),"The trace out bft sequence({}) is larger than the last execution sequence({}) for checkpoint interval({}),then try to move watermark.",index.sequence, bft_consensus.last_exe_sequence(), bft_consensus.ckp_interval());

                //We should move to the new watermark
                bft_consensus.set_view_active(true);
                bft_consensus.set_view_number(index.view_number);
                bft_consensus.set_last_exe_sequence(index.sequence);
                let last_exe_seq = index.sequence;

                // if bft_consensus.view_active() {
                //     BftStorage::store_value_i64(VIEW_ACTIVE, 1);
                // } else {
                //     BftStorage::store_value_i64(VIEW_ACTIVE, 0);
                // }

                //Clear the view change instance
                bft_consensus
                    .logs
                    .vc_instances
                    .retain(|_, value| !value.view_change_msg.has_bft());

                // BftStorage::store_vc_instances(&bft_consensus.logs.vc_instances);

                //Delete instance
                bft_consensus
                    .logs
                    .instances
                    .retain(|key, _| !(key.sequence <= last_exe_seq));

                //Clear the Out bft instance
                bft_consensus.logs.catch_up_instances.clear();
                bft_consensus.start_ledgerclose_check_timer();
            }
        }
    }
}
