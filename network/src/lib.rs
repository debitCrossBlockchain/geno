mod connections;
mod know_broadcast;
pub mod local_bus;
pub mod message_handler;
mod network_config;
pub mod node;
pub mod peer_network;
mod storage;
mod utils;

pub use node::Node;
/// A trait for objects containing a [`Node`]; it is required to implement protocols.
pub trait Peer2Peer {
    /// Returns a clonable reference to the node.
    fn node(&self) -> &Node;
}

pub use local_bus::{LocalBusBuilder, LocalBusPublisher, LocalBusSubscriber};
pub use message_handler::ReturnableProtocolsMessage;
pub use message_io::network::Endpoint;
pub use message_io::util::thread::NamespacedThread;
pub use network_config::NetworkConfigType;
pub use peer_network::PeerNetwork;
pub type SendMessageType = (Option<Endpoint>, protos::common::ProtocolsMessage);
