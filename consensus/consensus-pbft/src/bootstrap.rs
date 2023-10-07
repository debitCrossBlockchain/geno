use configure::CONFIGURE_INSTANCE_REF;
use crossbeam_channel::{bounded, Receiver};
use executor::LAST_COMMITTED_BLOCK_INFO_REF;
use ledger_upgrade::ledger_upgrade::{LedgerUpgradeInstance, LedgerUpgradeService};
use network::PeerNetwork;
use parking_lot::RwLock;
use protos::{
    common::{ProtocolsMessageType, TransactionResult},
    consensus::{BftSign, Consensus, ConsensusType},
    ledger::Ledger,
};
use std::sync::Arc;
use tracing::error;
use tx_pool::types::CommitNotificationSender;
use utils::{
    general::LEDGER_VERSION,
    parse::ProtocolParser,
    timer_manager::{TimerEventType, TimerManager, TimterEventParam},
};

use crate::bft_consensus::BftConsensus;

pub fn start_consensus(
    network_tx: PeerNetwork,
    network_consensus: PeerNetwork,
    commit_to_txpool_sender: CommitNotificationSender,
    ws_publish_event_sender: tokio::sync::mpsc::UnboundedSender<(Ledger, Vec<TransactionResult>)>,
) {
    let (timer_sender, timer_receiver) = bounded::<TimterEventParam>(1024);

    let validator_set = {
        LAST_COMMITTED_BLOCK_INFO_REF
            .read()
            .get_validators()
            .clone()
    };
    let lcl = { LAST_COMMITTED_BLOCK_INFO_REF.read().get_header().clone() };
    let ledger_upgrade_service =
        LedgerUpgradeService::start(network_consensus.clone(), lcl.get_height());
    let ledger_upgrade_instance = ledger_upgrade_service.ledger_upgrade.clone();

    let consensus = Arc::new(RwLock::new(BftConsensus::new(
        timer_sender,
        &validator_set,
        lcl.get_height(),
        ledger_upgrade_instance.clone(),
        network_tx.clone(),
        network_consensus.clone(),
        commit_to_txpool_sender,
        ws_publish_event_sender,
    )));

    if lcl.get_version() < LEDGER_VERSION {
        ledger_upgrade_instance
            .write()
            .set_new_version(LEDGER_VERSION);
    }
    if consensus.read().is_validator() {
        ledger_upgrade_instance.write().set_is_validator(true);
    }

    process(consensus.clone(), timer_receiver, network_consensus);
    start_consensus_check_timer(consensus.clone());
    start_consensus_publish_timer(consensus.clone());
    start_ledgerclose_check_timer(consensus.clone());
}

fn process(
    consensus: Arc<RwLock<BftConsensus>>,
    timer_receiver: Receiver<TimterEventParam>,
    network: PeerNetwork,
) {
    let subscriber = network.add_subscriber(ProtocolsMessageType::CONSENSUS);
    std::thread::spawn(move || loop {
        crossbeam_channel::select! {
            recv(timer_receiver) -> para =>{
                match para {
                    Ok(param)=>{
                        match param.event_type{
                            TimerEventType::PbftConsensusCheck => {
                                consensus.write().check_consensus_timeout(param.timestamp);
                            }
                            TimerEventType::PbftConsensusPublish => {
                                TimerManager::instance().delete_timer(param.id);
                                consensus.write().publish(&None);
                            }
                            TimerEventType::PbftLedgerCloseCheck => {
                                TimerManager::instance().delete_timer(param.id);
                                consensus.write().start_view_change();
                            }
                            TimerEventType::PbftNewViewRepond => {
                                TimerManager::instance().delete_timer(param.id);
                                if let Some(data) = param.data {
                                    consensus
                                        .write()
                                        .handle_new_view_repond_timer(data.as_slice());
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(e)=>{
                        error!("{:?}", e);
                    }
                }
            }
            recv(subscriber.inbox) -> msg =>{
                match msg {
                    Ok((_, msg))=>{
                        let (_, proto_message) = msg;
                        match ProtocolParser::deserialize::<Consensus>(proto_message.get_data()) {
                            Ok(consensus_message) =>{
                                match consensus_message.get_consensus_type() {
                                    ConsensusType::PBFT => {
                                        match ProtocolParser::deserialize::<BftSign>(consensus_message.get_msg()) {
                                            Ok(bft_sign) => {
                                                let _ = consensus.write().handle_receive_consensus(&bft_sign);
                                            }
                                            Err(e) => {
                                                error!("{:?}", e);
                                            }
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                error!("{:?}", e);
                            }
                        }
                    }
                    Err(e)=>{
                        error!("{:?}", e);
                    }
                }
            }
        }
    });
}

fn start_consensus_check_timer(consensus: Arc<RwLock<BftConsensus>>) {
    consensus.write().start_consensus_check_timer();
}

fn start_consensus_publish_timer(consensus: Arc<RwLock<BftConsensus>>) {
    consensus
        .write()
        .start_consensus_publish_timer(CONFIGURE_INSTANCE_REF.consensus.commit_interval);
}
fn start_ledgerclose_check_timer(consensus: Arc<RwLock<BftConsensus>>) {
    consensus.write().start_ledgerclose_check_timer();
}
