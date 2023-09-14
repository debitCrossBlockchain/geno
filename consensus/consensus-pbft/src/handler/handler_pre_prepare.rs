use crate::bft_check_value::CheckValueResult;
use crate::bft_state::BftInstancePhase::{NONE, PRE_PREPARED};
use crate::bft_state::BftState;
use crate::instance::bft_instance::BftInstance;
use crate::new_bft_message::NewBftMessage;
use protos::consensus::Bft;
use tracing::*;
use utils::general::hash_crypto_byte;

pub struct HandlerPrePrepare {}

impl HandlerPrePrepare {
    pub fn handle_pre_prepare(
        state: &mut BftState,
        bft: &Bft,
        instance: &mut BftInstance,
        check_value: CheckValueResult,
    ) -> bool {
        //Continue if it has only one node,in solo.
        if state.is_solo() {
            return true;
        }
        let pre_prepare = bft.get_pre_prepare();
        //Check the value digest
        let hash = hash_crypto_byte(pre_prepare.get_value());
        if hash != pre_prepare.get_value_digest() {
            error!(parent:state.span(),
                "The value digest ({}) is not equal to ({})'s digest, desc({})",
                msp::bytes_to_hex_str(pre_prepare.get_value_digest()),
                NewBftMessage::consensus_value_desc(pre_prepare.get_value()),
                NewBftMessage::bft_desc(bft)
            );
            return false;
        }

        //Check the value
        if check_value == CheckValueResult::InValid {
            error!(parent:state.span(),
                "Failed to check the value({}), desc({})",
                NewBftMessage::consensus_value_desc(pre_prepare.get_value()),
                NewBftMessage::bft_desc(bft)
            );
            return false;
        }

        if instance.phase != NONE {
            return Self::handle_pre_prepare_again(state, bft, instance, check_value);
        }

        info!(parent:state.span(),"Received pre-prepare message from {}, round number({}), value({})",
            NewBftMessage::base_info_desc(pre_prepare.get_base()), bft.get_round_number(),
			NewBftMessage::consensus_value_desc(pre_prepare.get_value()));

        //Insert the instance to map
        instance.phase.clone_from(&PRE_PREPARED);
        instance.phase_item = 0;
        instance.pre_prepare.clone_from(pre_prepare);
        instance.check_value.clone_from(&check_value);

        if check_value != CheckValueResult::Valid {
            info!(parent:state.span(),"Failed to check the value({}, round number:1, value:{}), so don't send prepare message",NewBftMessage::base_info_desc(pre_prepare.get_base()),NewBftMessage::consensus_value_desc(pre_prepare.get_value()));
            return true;
        }

        let prepare_msg = NewBftMessage::new_prepare(state, pre_prepare, 1);
        info!(parent:state.span(),
            "Send prepare message: {}, round number(1), value({})",
            NewBftMessage::base_info_desc(prepare_msg.get_bft().get_prepare().get_base()),
            NewBftMessage::consensus_value_desc(pre_prepare.get_value())
        );
        state.broadcast_message(&prepare_msg);
        true
    }

    pub fn handle_pre_prepare_again(
        state: &mut BftState,
        bft: &Bft,
        instance: &BftInstance,
        check_value: CheckValueResult,
    ) -> bool {
        let pre_prepare = bft.get_pre_prepare();
        if instance.pre_prepare.get_value() != pre_prepare.get_value() {
            error!(parent:state.span(),
                "The pre-prepare message value({}) != this value({}) , desc({})",
                NewBftMessage::consensus_value_desc(pre_prepare.get_value()),
                NewBftMessage::consensus_value_desc(instance.pre_prepare.get_value()),
                NewBftMessage::bft_desc(bft)
            );
            return false;
        }

        info!(parent:state.span(),"The message value({}) received is duplicated, desc({})",
        NewBftMessage::consensus_value_desc(pre_prepare.get_value()), NewBftMessage::bft_desc(bft));

        if check_value != CheckValueResult::Valid {
            info!(parent:state.span(),"Failed to check the value(view number:{},sequence:{}, round number:1), so don't send prepare message.", pre_prepare.get_base().get_view_number(), pre_prepare.get_base().get_sequence());
            return true;
        }

        info!(parent:state.span(),
            "Send prepare message again: view number({}), sequence({}), round number({})",
            pre_prepare.get_base().get_view_number(),
            pre_prepare.get_base().get_sequence(),
            bft.get_round_number()
        );
        let prepare_msg = NewBftMessage::new_prepare(state, pre_prepare, bft.get_round_number());
        state.broadcast_message(&prepare_msg);
        true
    }
}
