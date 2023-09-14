use crate::bft_state::BftInstancePhase::COMMITTED;
use crate::bft_state::BftState;
use crate::instance::bft_instance::BftInstance;
use crate::new_bft_message::NewBftMessage;
use protobuf::Message;
use protos::consensus::Bft;
use tracing::*;
pub struct HandlerCommit {}

impl HandlerCommit {
    pub fn handle_commit(
        state: &mut BftState,
        bft: &Bft,
        instance: &mut BftInstance,
        trigger_committed: &mut bool,
    ) -> bool {
        if !bft.has_commit()
            || bft.get_commit().compute_size() == 0
            || (bft.get_commit().get_base().get_view_number() == 0
                && bft.get_commit().get_base().get_sequence() == 0)
        {
            error!(parent:state.span(),"bft has no commit feild");
            return false;
        }
        if instance.pre_prepare.compute_size() == 0
            || (instance.pre_prepare.get_base().get_view_number() == 0
                && instance.pre_prepare.get_base().get_sequence() == 0)
        {
            error!(parent:state.span(),"instance has not pre_prepare feild");
            return false;
        }

        let commit = bft.get_commit();
        if instance.pre_prepare.get_value_digest() != commit.get_value_digest() {
            error!(parent:state.span(),
                "The commit message digest({}) != this pre-prepare digest({}), pre-prepare value desc({}),commit desc({})",
                msp::bytes_to_hex_str(commit.get_value_digest()),
                msp::bytes_to_hex_str(instance.pre_prepare.get_value_digest()),
                NewBftMessage::consensus_value_desc(instance.pre_prepare.get_value()), NewBftMessage::bft_desc(bft)
            );
            return false;
        }

        if instance
            .commits
            .contains_key(&commit.get_base().get_replica_id())
        {
            info!(parent:state.span(),
                "The commit message {} has been received and duplicated",
                NewBftMessage::base_info_desc(commit.get_base())
            );
            return true;
        }

        info!(parent:state.span(),"Received commit message from {}, round number({})",
        NewBftMessage::base_info_desc(commit.get_base()), bft.get_round_number());
        instance
            .commits
            .entry(commit.get_base().get_replica_id())
            .or_insert(commit.clone());
        if instance.is_committing(state) {
            instance.phase.clone_from(&COMMITTED);
            instance.phase_item = 0;
            instance.end_time = chrono::Local::now().timestamp_millis();
            instance.commit_complete = true;
            *trigger_committed = true;
            info!(parent:state.span(),
                "committed!! view_number({}) sequence({}) try to execute consensus value.",
                instance.pre_prepare.get_base().get_view_number(),
                instance.pre_prepare.get_base().get_sequence()
            );
            // This consensus has achieved.
            //return state.execute_value();
        }
        return true;
    }
}
