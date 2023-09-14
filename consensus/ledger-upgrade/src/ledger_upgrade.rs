use crossbeam_channel::{bounded, select, Receiver};
use fxhash::FxHashMap;
use network::message_handler::ReturnableProtocolsMessage;
use network::peer_network::PeerNetwork;
use network::{LocalBusPublisher, LocalBusSubscriber};
use parking_lot::RwLock;
use protos::common::{
    ProtocolsActionMessageType, ProtocolsMessage, ProtocolsMessageType, ValidatorSet,
};

use protos::consensus::{LedgerUpgrade, LedgerUpgradeInfo, LedgerUpgradeNotify};
use std::{collections::HashSet, sync::Arc};

use tracing::*;
use utils::{
    general::*,
    parse::ProtocolParser,
    timer_manager::{TimerEventType, TimerManager, TimterEventParam},
    verify_sign::{get_sign_address, verify_sign},
};

pub struct LedgerUpgradeInstance {
    pub last_send_time: i64,
    pub last_ledger_version: u64,
    pub local_state: LedgerUpgrade,
    pub current_states: FxHashMap<String, LedgerUpgradeInfo>,
    pub timer: Receiver<TimterEventParam>,
    pub is_validator: bool,
    pub network: PeerNetwork,
    pub publisher: LocalBusPublisher<ProtocolsMessageType, ReturnableProtocolsMessage>,
    pub subscriber: LocalBusSubscriber<ProtocolsMessageType, ReturnableProtocolsMessage>,
}

impl LedgerUpgradeInstance {
    pub fn new(
        network: PeerNetwork,
        publisher: LocalBusPublisher<ProtocolsMessageType, ReturnableProtocolsMessage>,
        subscriber: LocalBusSubscriber<ProtocolsMessageType, ReturnableProtocolsMessage>,
        last_ledger_version: u64,
        timer: Receiver<TimterEventParam>,
    ) -> LedgerUpgradeInstance {
        LedgerUpgradeInstance {
            last_send_time: chrono::Local::now().timestamp_millis() - 30000,
            last_ledger_version,
            local_state: LedgerUpgrade::default(),
            current_states: FxHashMap::default(),
            timer,
            is_validator: false,
            network,
            publisher,
            subscriber,
        }
    }

    pub fn is_validator(&self) -> bool {
        self.is_validator
    }

    pub fn set_is_validator(&mut self, is_validator: bool) {
        self.is_validator = is_validator;
    }

    pub fn handle_receive(&mut self, msg: &LedgerUpgradeNotify) {
        let upgrade = msg.get_upgrade();
        let sig = msg.get_signature();

        let mut notify = LedgerUpgradeNotify::default();
        notify.set_nonce(msg.get_nonce());
        notify.set_upgrade(upgrade.clone());

        match verify_sign(sig, ProtocolParser::serialize(&notify).as_slice()) {
            Ok(value) => {
                if !value {
                    error!("verify_sign failed in ledger upgrade");
                    return;
                }
            }
            Err(e) => {
                error!("{:?} in ledger upgrade", e);
                return;
            }
        }

        if upgrade.get_chain_id() != self_chain_id() {
            error!(
                "Failed to check same chain, node self id({:?}) is not eq ({:?})",
                self_chain_id(),
                upgrade.get_chain_id()
            );
            return;
        }

        if upgrade.get_chain_hub() != self_chain_hub() {
            error!(
                "Failed to check same chain, node self hub({:?}) is not eq ({:?})",
                self_chain_hub(),
                upgrade.get_chain_hub()
            );
            return;
        }

        if let Ok(addr) = get_sign_address(sig) {
            let mut info = LedgerUpgradeInfo::default();
            info.set_address(addr.clone());
            info.set_recv_time(chrono::Local::now().timestamp_millis());
            info.set_msg(msg.clone());

            self.current_states.insert(addr.clone(), info);
        }
    }

    pub fn set_last_ledger_version(&mut self, version: u64) {
        self.last_ledger_version = version;
    }

    pub fn set_new_version(&mut self, new_version: u64) {
        self.local_state.set_new_version(new_version);
        self.local_state.set_chain_id(self_chain_id());
        self.local_state.set_chain_hub(self_chain_hub());
    }

    pub fn get_valid(
        &mut self,
        validators: &ValidatorSet,
        quorum_size: usize,
    ) -> Option<LedgerUpgrade> {
        if self.current_states.is_empty() {
            return None;
        }

        let mut validator_set: HashSet<String> = HashSet::default();
        for i in validators.get_validators() {
            validator_set.insert(i.get_address().to_string());
        }

        let mut counter_upgrade: FxHashMap<Vec<u8>, i32> = FxHashMap::default();
        for (addr, info) in self.current_states.iter() {
            let hash = hash_crypto(&ProtocolParser::serialize::<LedgerUpgrade>(
                info.get_msg().get_upgrade(),
            ));
            if !counter_upgrade.contains_key(&hash) {
                counter_upgrade.insert(hash.clone(), 0);
            }

            if validator_set.contains(addr) {
                if let Some(v) = counter_upgrade.get(&hash) {
                    let value = *v;
                    counter_upgrade.insert(hash.clone(), value + 1);
                    if value as usize + 1 >= quorum_size {
                        return Some(info.get_msg().get_upgrade().clone());
                    }
                }
            }
        }
        None
    }

    pub fn handle_timer(&mut self, current_time: i64) {
        //Delete the expired
        self.current_states
            .retain(|_, v| (v.recv_time + 300 * MILLI_UNITS_PER_SEC) >= current_time);

        //Send the current state every 30s
        let mut notify = LedgerUpgradeNotify::default();
        if ((current_time - self.last_send_time) > (30 * MILLI_UNITS_PER_SEC))
            && self.local_state.get_new_version() > 0
            && self.local_state.get_new_version() > self.last_ledger_version
        {
            notify.set_nonce(current_time);
            notify.set_upgrade(self.local_state.clone());
            let sig = match utils::verify_sign::sign(
                &node_private_key(),
                ProtocolParser::serialize(&notify).as_slice(),
            ) {
                Ok(value) => value,
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };
            notify.set_signature(sig);
            self.last_send_time = current_time;

            if self.is_validator() {
                self.broadcast_message(&notify);
            }
        }
    }

    pub fn broadcast_message(&self, message: &LedgerUpgradeNotify) -> bool {
        if !self.is_validator() {
            return true;
        }
        let mut msg = ProtocolsMessage::new();
        msg.set_msg_type(ProtocolsMessageType::LEDGER_UPGRADE_NOTIFY);
        msg.set_action(ProtocolsActionMessageType::BROADCAST);
        msg.set_data(ProtocolParser::serialize(message));

        let _ = self.network.broadcast_msg(msg.clone());

        self.publisher.publish(
            ProtocolsMessageType::LEDGER_UPGRADE_NOTIFY,
            (self.network.listen_endpoint(), msg),
        );
        true
    }

    pub fn process(&mut self) {
        select! {
            recv(self.timer) -> msg =>{
                match msg {
                    Ok(param)=>{
                        match param.event_type{
                            TimerEventType::LedgerUpgrade =>{
                                let now = chrono::Local::now().timestamp_millis();
                                self.handle_timer(now);
                            }
                            _=>{}
                        }
                    }
                    Err(e)=>{
                        error!("{:?}", e);
                    }
                }
            }
            recv(self.subscriber.inbox) -> msg =>{
                match msg {
                    Ok((_, msg))=>{
                        let (_, proto_message) = msg;
                        match ProtocolParser::deserialize::<LedgerUpgradeNotify>(proto_message.get_data()) {
                            Ok(notify) => self.handle_receive(&notify),
                            Err(e) => {
                                error!("{:?} in ledger upgrade", e);
                            }
                        }
                    }
                    Err(e)=>{
                        error!("{:?}", e);
                    }
                }
            }
        }
    }
}

pub struct LedgerUpgradeService {
    pub ledger_upgrade: Arc<RwLock<LedgerUpgradeInstance>>,
    pub task: std::thread::JoinHandle<()>,
}

impl LedgerUpgradeService {
    pub fn start(network: PeerNetwork, last_ledger_version: u64) -> LedgerUpgradeService {
        let publisher = network.publisher();
        let subscriber = network.add_subscriber(ProtocolsMessageType::LEDGER_UPGRADE_NOTIFY);

        let (sender, recver) = bounded(1024);
        let _ = TimerManager::instance().new_repeating_timer(
            chrono::Duration::seconds(2),
            sender,
            TimerEventType::LedgerUpgrade,
            None,
        );

        let ledger_upgrade = Arc::new(RwLock::new(LedgerUpgradeInstance::new(
            network.clone(),
            publisher,
            subscriber,
            last_ledger_version,
            recver,
        )));

        let ledger_upgrade_clone = ledger_upgrade.clone();
        let task = std::thread::spawn(move || loop {
            ledger_upgrade_clone.write().process();
        });

        let service = LedgerUpgradeService {
            ledger_upgrade,
            task,
        };

        service
    }
}
