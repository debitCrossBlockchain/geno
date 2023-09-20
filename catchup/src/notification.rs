
use crossbeam_channel::{Receiver, Sender};
use network::Endpoint;
use protos::common::ProtocolsMessage;
use utils::timer_manager::TimterEventParam;
use futures::{
    channel::{mpsc, mpsc::UnboundedSender, oneshot},
    future::Future,
    task::{Context, Poll},
};
use types::SignedTransaction;


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

pub enum TxpoolNotification{
    TxnsBroadcast(Vec<SignedTransaction>),
}

pub type  ClientNotificationReceiver = Receiver<ClientNotification>;
//pub type CatchupNotificationReceiver = Receiver<CatchupNotification>; //
pub type CatchupNotificationReceiver = Receiver<(Endpoint, ProtocolsMessage)>; 
//pub type TimerNotificationReceiver = Receiver<TimerNotification>;
pub type TimerNotificationReceiver = Receiver<TimterEventParam>;
pub type TxpoolNotificationSender = Sender<TxpoolNotification>;

pub type BroadcastSender = mpsc::UnboundedSender<Vec<SignedTransaction>>;

