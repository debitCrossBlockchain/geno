
use crossbeam_channel::Receiver;
use network::Endpoint;
use protos::common::ProtocolsMessage;
use utils::timer_manager::TimterEventParam;

pub enum ClientNotification{
    GetTransaction(),
}

pub enum CatchupNotification{
    BlockCatchupRequest(),
    BlockCatchupResponse(),
    HeaderCatchupRequest(),
    HeaderCatchupResponse(),
}

pub enum TimerNotification{
    BlockCatchup,
}

pub type  ClientNotificationReceiver = Receiver<ClientNotification>;
//pub type CatchupNotificationReceiver = Receiver<CatchupNotification>; //
pub type CatchupNotificationReceiver = Receiver<(Endpoint, ProtocolsMessage)>; 
//pub type TimerNotificationReceiver = Receiver<TimerNotification>;
pub type TimerNotificationReceiver = Receiver<TimterEventParam>;

