use crate::bft_consensus::BftConsensus;
use crate::bft_state::BftInstancePhase::{COMMITTED, PREPARED};
use crate::instance::bft_vc_instance::BftVcInstance;
use crate::new_bft_message::NewBftMessage;
use consensus_store::bft_storage::BftStorage;
use protos::consensus::BftSign;
use std::collections::HashSet;
use tracing::*;

pub struct HandlerNewView {}

impl HandlerNewView {
    pub fn handler_new_view(bft_consensus: &mut BftConsensus, bft_sign: &BftSign) -> bool {
        let bft = bft_sign.get_bft();
        let new_view = bft.get_new_view();
        let validators_size = bft_consensus.state.validators.len() as i64;
        let view_number = bft_consensus.view_number();
        let replica_id = bft_consensus.replica_id();
        let last_exe_seq = bft_consensus.last_exe_sequence();

        info!(parent:bft_consensus.span(),
            "handler_new_view Received view change message from {},round number({})",
            NewBftMessage::base_info_desc(new_view.get_base()),
            bft.get_round_number()
        );
        if new_view.get_base().get_view_number() == view_number {
            info!(parent:bft_consensus.span(),
                "handler_new_view The new view number({}) is equal to current view number, then do nothing",
                new_view.get_base().get_view_number()
            );
            return true;
        } else if new_view.get_base().get_view_number() < view_number {
            info!(parent:bft_consensus.span(),
                "handler_new_view The new view number({}) is less than current view number({}), then do nothing",
                new_view.get_base().get_view_number(),
                view_number
            );
            return false;
        }

        //Delete the response timer
        bft_consensus.delete_new_view_repond_timer();

        if (new_view.get_base().get_view_number() % validators_size) == replica_id {
            info!(parent:bft_consensus.span(),
                "handler_new_view It's the new primary(replica_id:{}), so do not process the new view message",
                replica_id
            );
            return true;
        }

        //Check the view change message
        let mut vc_tmp = BftVcInstance::new();
        vc_tmp.view_number = new_view.get_base().get_view_number();

        let mut check_ret = true;
        let mut replica_set: HashSet<i64> = HashSet::new();

        for iter in new_view.get_view_changes() {
            vc_tmp.msg_buf.push(iter.clone());
            if !bft_consensus.state.check_bft_message(bft_sign) {
                check_ret = false;
                break;
            }

            let view_change = iter.get_bft().get_view_change();
            vc_tmp
                .view_changes
                .insert(view_change.get_base().get_replica_id(), view_change.clone());

            if new_view.get_base().get_view_number() != view_change.get_base().get_view_number() {
                error!(parent:bft_consensus.span(),"handler_new_view The new view message's view-number({}) is not equal to it's view-change number({})",
                    new_view.get_base().get_view_number(), view_change.get_base().get_view_number());
                check_ret = false;
                break;
            }

            replica_set.insert(view_change.get_base().get_replica_id());
        }

        if !check_ret {
            return false;
        }

        let quorum_size = bft_consensus.state.quorum_size();
        if replica_set.len() <= quorum_size {
            error!(parent:bft_consensus.span(),"handler_new_view The new view message(number:{})'s count({}) is less than or equal to quorum size({})",
                      new_view.get_base().get_view_number(), replica_set.len(),quorum_size);
            return false;
        }

        //Delete the other log
        bft_consensus.logs.instances.retain(|key, value| {
            (value.phase == COMMITTED) || (value.phase == PREPARED) && (key.sequence > last_exe_seq)
        });

        info!(parent:bft_consensus.span(),
            "handler_new_view replica_id({}) enter the new view_number({})",
            replica_id,
            new_view.get_base().get_view_number()
        );

        //Enter the new view
        bft_consensus.set_view_number(new_view.get_base().get_view_number());
        bft_consensus.set_view_active(true);
        BftStorage::store_view_number(bft_consensus.view_number());
        // BftStorage::store_value_i64(VIEW_ACTIVE, 1);

        bft_consensus
            .logs
            .change_vc_instance_complete(bft_consensus.view_number());
        bft_consensus.clear_view_changes();
        info!(parent:bft_consensus.span(),"handler_new_view trace-consensus handle_view_changed",);
        bft_consensus.handle_view_changed(&None);
        return true;
    }
}
