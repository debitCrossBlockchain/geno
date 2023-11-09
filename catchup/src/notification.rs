use crossbeam_channel::{Receiver, Sender};
use futures::{
    channel::{mpsc, mpsc::UnboundedSender, oneshot},
    future::Future,
    task::{Context, Poll},
};
use network::Endpoint;
use protos::{common::ProtocolsMessage, ledger::Ledger};
use types::SignedTransaction;
use utils::timer_manager::TimterEventParam;

pub enum ClientNotification {
    GetTransaction(),
}

pub enum CatchupNotification {
    BlockCatchupRequest(),
    BlockCatchupResponse(),
    HeaderCatchupRequest(),
    HeaderCatchupResponse(),
}

pub enum TimerNotification {
    BlockCatchup,
}

pub enum TxpoolNotification {
    TxnsBroadcast(Vec<SignedTransaction>),
}

pub type ClientNotificationReceiver = Receiver<ClientNotification>;
//pub type CatchupNotificationReceiver = Receiver<CatchupNotification>; //
pub type ChainStatusReceiver = Receiver<(Endpoint, ProtocolsMessage)>;
pub type ChainStatusSender = Sender<(Endpoint, ProtocolsMessage)>;
pub type BlocksReceiver = Receiver<(Endpoint, ProtocolsMessage)>;
pub type BlocksSender = Sender<(Endpoint, ProtocolsMessage)>;
//pub type TimerNotificationReceiver = Receiver<TimerNotification>;
pub type TimerNotificationReceiver = Receiver<TimterEventParam>;
pub type TxpoolNotificationSender = Sender<TxpoolNotification>;

pub type CommitBlockSender = Sender<Ledger>;

pub type BroadcastSender = mpsc::UnboundedSender<Vec<SignedTransaction>>;
