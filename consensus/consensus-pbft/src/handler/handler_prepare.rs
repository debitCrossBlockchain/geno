use crate::bft_check_value::CheckValueResult;
use crate::bft_state::BftInstancePhase::PREPARED;
use crate::bft_state::BftState;
use crate::instance::bft_instance::BftInstance;
use crate::new_bft_message::NewBftMessage;
use protobuf::Message;
use protos::consensus::Bft;
use tracing::*;

pub struct HandlerPrepare {}

impl HandlerPrepare {
    pub fn handle_prepare(state: &mut BftState, bft: &Bft, instance: &mut BftInstance) -> bool {
        if !bft.has_prepare()
            || bft.get_prepare().compute_size() == 0
            || (bft.get_prepare().get_base().get_view_number() == 0
                && bft.get_prepare().get_base().get_sequence() == 0)
        {
            error!(parent:state.span(),"bft has no prepare feild");
            return false;
        }
        if instance.pre_prepare.compute_size() == 0
            || (instance.pre_prepare.get_base().get_view_number() == 0
                && instance.pre_prepare.get_base().get_sequence() == 0)
        {
            error!(parent:state.span(),"instance has no pre_prepare feild");
            return false;
        }

        let prepare = bft.get_prepare();
        if instance.pre_prepare.get_value_digest() != prepare.get_value_digest() {
            error!(parent:state.span(),
                "The message prepare digest({}) != this pre-prepare digest({}), desc({})",
                msp::bytes_to_hex_str(prepare.get_value_digest()),
                msp::bytes_to_hex_str(instance.pre_prepare.get_value_digest()),
                NewBftMessage::bft_desc(bft)
            );
            return false;
        }

        info!(parent:state.span(),"Received prepare message from {}, round number({})",NewBftMessage::base_info_desc(prepare.get_base()), bft.get_round_number());

        let mut exist_msg = "";
        if instance
            .prepares
            .contains_key(&prepare.get_base().get_replica_id())
        {
            exist_msg = " again";
        }

        instance
            .prepares
            .entry(prepare.get_base().get_replica_id())
            .or_insert(prepare.clone());
        if instance.prepares.len() < state.quorum_size() {
            return true;
        }

        if (instance.phase.clone() as i64) < (PREPARED as i64) {
            //Detect and receive again
            instance.phase.clone_from(&PREPARED);
            instance.phase_item = 0;
        }

        //Send commit
        if instance.check_value == CheckValueResult::Valid {
            info!(parent:state.span(),
                "Sending commit message {}, view number({}), sequence({}), round number({})",
                exist_msg,
                instance.pre_prepare.get_base().get_view_number(),
                instance.pre_prepare.get_base().get_sequence(),
                bft.get_round_number()
            );
            let commit_msg = NewBftMessage::new_commit(state, prepare, bft.get_round_number());
            instance.set_last_commit_send_time(chrono::Local::now().timestamp_millis());
            state.broadcast_message(&commit_msg);
        } else {
            info!(parent:state.span(),"Don't send commit message(msg value:{}, view number:{}, sequence:{}, round number:{}) because the check result is not valid.",
                exist_msg,instance.pre_prepare.get_base().get_view_number(), instance.pre_prepare.get_base().get_sequence(), bft.get_round_number());
        }
        true
    }
}
