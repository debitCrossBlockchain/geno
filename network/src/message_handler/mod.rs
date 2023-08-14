use anyhow::*;
use crossbeam_channel::{bounded, Receiver, Sender};
use message_io::network::Endpoint;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use protos::common::ProtocolsMessage;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
pub mod echo_message_handler;

#[derive(Default)]
pub(crate) struct ProtocolsMessageHandler {
    pub(crate) echo_message_handler: OnceCell<ProtocolMessageHandler>,
    pub(crate) req_resp_handler:
        Arc<RwLock<HashMap<protos::common::ProtocolsMessageType, ProtocolMessageHandler>>>,
}

/// An object dedicated to managing a protocol; it contains a `Sender` whose other side is
/// owned by the protocol's task, a handle to which is also held by the `ProtocolHandler`.
pub struct ProtocolMessageHandler {
    sender: Sender<ReturnableProtocolsMessage>,
}

impl ProtocolMessageHandler {
    /// Sends a returnable `ProtocolsMessageHandle` to a task spawned by the protocol handler.
    pub fn send(&self, returnable_conn: ReturnableProtocolsMessage) -> Result<()> {
        if self.sender.try_send(returnable_conn).is_err() {
            return Err(anyhow::anyhow!("A protocol handler's Receiver is closed"));
        }
        Ok(())
    }
}

impl From<Sender<ReturnableProtocolsMessage>> for ProtocolMessageHandler {
    fn from((sender): Sender<ReturnableProtocolsMessage>) -> Self {
        Self { sender }
    }
}

/// An object allowing a `ProtocolsMessage` to be "borrowed" from the owning `peer_network` to enable a protocol
/// and to be sent back to it once it's done its job.
pub type ReturnableProtocolsMessage = (Endpoint, ProtocolsMessage);
