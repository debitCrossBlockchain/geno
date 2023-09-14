use crate::bft_check_value::CheckValueResult;
use crate::bft_state::BftState;
use crate::handler::handler_commit::HandlerCommit;
use crate::handler::handler_pre_prepare::HandlerPrePrepare;
use crate::handler::handler_prepare::HandlerPrepare;
use crate::instance::bft_instance::BftInstance;
use protos::consensus::{BftMessageType, BftSign};
use tracing::*;
pub enum HandlerInstance {}

impl HandlerInstance {
    pub fn handler_primary(
        instance: &mut BftInstance,
        bft_sign: &BftSign,
        state: &mut BftState,
        check_value: CheckValueResult,
        trigger_committed: &mut bool,
    ) -> bool {
        info!(parent:state.span(),
            "handler_primary receive message type({:?}) for instance phase({:?})",
            bft_sign.get_bft().get_msg_type(),
            instance.phase
        );
        if (bft_sign.get_bft().get_msg_type() as i64) < (instance.phase.clone() as i64) {
            //It is received again
            match bft_sign.get_bft().get_msg_type() {
                BftMessageType::PRE_PREPARE => {
                    HandlerPrePrepare::handle_pre_prepare(
                        state,
                        bft_sign.get_bft(),
                        instance,
                        check_value,
                    );
                }
                BftMessageType::PREPARE => {
                    HandlerPrepare::handle_prepare(state, bft_sign.get_bft(), instance);
                }
                BftMessageType::COMMIT => {
                    HandlerCommit::handle_commit(
                        state,
                        bft_sign.get_bft(),
                        instance,
                        trigger_committed,
                    );
                }
                _ => {}
            }
            return false;
        }

        let mut ret = false;
        while instance.get_bft_sign_vec(&instance.phase.clone()).len()
            > (instance.phase_item as usize)
        {
            if let Some(item) = instance.get_bft_sign(&instance.phase, instance.phase_item as usize)
            {
                let bft = item.get_bft();
                info!(parent:state.span(),
                    "handler_primary instance phase({:?}) phase_item({}) message type({:?})",
                    instance.phase,
                    instance.phase_item,
                    bft.get_msg_type()
                );
                instance.phase_item += 1;

                match bft.get_msg_type() {
                    BftMessageType::PRE_PREPARE => {
                        ret = HandlerPrePrepare::handle_pre_prepare(
                            state,
                            bft,
                            instance,
                            check_value,
                        );
                    }
                    BftMessageType::PREPARE => {
                        ret = HandlerPrepare::handle_prepare(state, bft, instance);
                    }
                    BftMessageType::COMMIT => {
                        ret = HandlerCommit::handle_commit(state, bft, instance, trigger_committed);
                    }
                    _ => {}
                }
            } else {
                error!(parent:state.span(),
                    "handler_primary can not find message by phase({:?}) phase_item({}) in instance",
                    instance.phase, instance.phase_item
                );
            }
        }
        return ret;
    }
}
