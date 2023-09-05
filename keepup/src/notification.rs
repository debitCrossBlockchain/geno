
use crossbeam_channel::Receiver;
use network::Endpoint;
use protos::common::ProtocolsMessage;
use utils::timer_manager::TimterEventParam;

pub enum ClientNotification{
    GetTransaction(),
}

pub enum SyncNotification{
    BlockSyncRequest(),
    BlockSyncResponse(),
    HeaderSyncRequest(),
    HeaderSyncResponse(),
}

pub enum TimerNotification{
    BlockSync,
}

pub type  ClientNotificationReceiver = Receiver<ClientNotification>;
//pub type SyncNotificationReceiver = Receiver<SyncNotification>; //
pub type SyncNotificationReceiver = Receiver<(Endpoint, ProtocolsMessage)>; 
//pub type TimerNotificationReceiver = Receiver<TimerNotification>;
pub type TimerNotificationReceiver = Receiver<TimterEventParam>;

