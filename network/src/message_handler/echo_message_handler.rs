use crate::{connections::ConnectionSide, storage::P2PStorage, utils::PEER_DB_COUNT, PeerNetwork};
use message_io::network::Endpoint;
use protobuf::Message;
use protos::common::{
    ErrCode, HelloMessage, HelloResponseMessage, Peer, Peers, ProtocolsActionMessageType,
    ProtocolsMessage, ProtocolsMessageType,
};
use std::{net::SocketAddr, str::FromStr};
use tracing::*;
///
pub struct HelloMessageHandler;

impl HelloMessageHandler {
    ///
    pub fn send_hello_request(network: &PeerNetwork, peer_id: Endpoint) {
        let mut message = HelloMessage::default();
        let config = network.network_config();
        message.set_ledger_version(36002);
        message.set_network_version(36003);
        message.set_node_address(config.node_address.clone());
        message.set_node_rand(config.node_rand.clone());
        message.set_network_id(config.network_id.clone() as u64);
        message.set_chain_id(config.chain_id.clone());
        message.set_chain_hub(config.chain_hub.clone());
        message.set_listening_port(config.listen_addr.port() as i64);

        let mut msg = ProtocolsMessage::new();
        msg.set_msg_type(ProtocolsMessageType::HELLO);
        msg.set_action(ProtocolsActionMessageType::REQUEST);
        msg.set_data(message.write_to_bytes().unwrap());

        info!(parent:network.span(),
            "({}) send hello request(listen port {})",
            config.local_addr,
            config.listen_addr.port()
        );
        network.send_msg(peer_id, msg);
    }

    ///
    pub fn send_hello_response(
        network: &PeerNetwork,
        peer_id: Endpoint,
        response: HelloResponseMessage,
    ) {
        let mut msg = ProtocolsMessage::new();
        msg.set_msg_type(ProtocolsMessageType::HELLO);
        msg.set_action(ProtocolsActionMessageType::RESPONSE);
        msg.set_data(response.write_to_bytes().unwrap());
        network.send_msg(peer_id, msg);
    }

    ///
    pub fn handle_hello_request(
        network: &PeerNetwork,
        peer_addr: Endpoint,
        proto_message: &ProtocolsMessage,
    ) {
        let config = network.network_config();
        let message: HelloMessage =
            protobuf::Message::parse_from_bytes(proto_message.get_data()).unwrap();

        let mut response = HelloResponseMessage::default();
        Self::handshaking_check(network, peer_addr, &message, &mut response);
        Self::send_hello_response(network, peer_addr.clone(), response);
    }

    /// check hello request
    pub fn handshaking_check(
        network: &PeerNetwork,
        peer_addr: Endpoint,
        message: &HelloMessage,
        response: &mut HelloResponseMessage,
    ) {
        let config = network.network_config();
        if message.chain_hub != config.chain_hub {
            response.set_err_code(ErrCode::ERRCODE_INVALID_PARAMETER);
            let err_info = format!(
                "different chain_hub, remote chain_hub {} is not equal to the local chain_hub {}",
                message.chain_hub, config.chain_hub
            );
            response.set_err_desc(err_info.clone());
            error!(
                "failed to process the peer handshaking message {}",
                err_info.clone()
            );
            return;
        }

        if message.network_id != config.network_id.clone() as u64 {
            response.set_err_code(ErrCode::ERRCODE_INVALID_PARAMETER);
            let err_info = format!("different network_id, remote network_id {} is not equal to the local network_id {}", message.network_id, config.network_id);
            response.set_err_desc(err_info.clone());
            error!(
                "failed to process the peer handshaking message {}",
                err_info.clone()
            );
            return;
        }

        if message.chain_id != config.chain_id {
            response.set_err_code(ErrCode::ERRCODE_INVALID_PARAMETER);
            let err_info = format!(
                "different chain_id, remote chain_id {} is not equal to the local chain_id {}",
                message.chain_id, config.chain_id
            );
            response.set_err_desc(err_info.clone());
            error!(
                "failed to process the peer handshaking message {}",
                err_info.clone()
            );
            return;
        }

        let listen_port = message.get_listening_port();
        if listen_port <= 0 || listen_port > 65535 {
            response.set_err_code(ErrCode::ERRCODE_INVALID_PARAMETER);
            let err_info = format!("peer's listen port {} is not valid", listen_port);
            response.set_err_desc(err_info.clone());
            error!(
                "failed to process the peer handshaking message {}",
                err_info.clone()
            );
            return;
        }

        if message.node_address == config.node_address {
            response.set_err_code(ErrCode::ERRCODE_INVALID_PARAMETER);
            if message.get_node_rand() != config.node_rand.as_str() {
                let err_info = format!(
                    "the peer connection breaks as the configuration node addresses are duplicated"
                );
                response.set_err_desc(err_info.clone());
                error!(
                    "failed to process the peer handshaking message {}",
                    err_info.clone()
                );
            } else {
                let err_info = format!("The peer connection is broken because it connects itself");
                response.set_err_desc(err_info.clone());
                error!(
                    "failed to process the peer handshaking message {}",
                    err_info.clone()
                );
            }
            return;
        }

        let mut remote_addr = peer_addr.addr();
        remote_addr.set_port(listen_port as u16);
        network.set_remote_info(&peer_addr, remote_addr, message.node_address.clone());

        let conn_side = network.conn_side(&peer_addr);

        info!(
            "({}) handle hello request,receive listening port({}) ,peer_addr({})--remote_addr({}) ,conn_side({:?})",
            config.local_addr,listen_port, peer_addr,remote_addr, conn_side
        );

        match conn_side {
            ConnectionSide::Initiator => {
                Self::send_hello_request(network, peer_addr.clone());
                PeerSyncMessageHandler::send_peersync_request(network, peer_addr.clone());
            }
            _ => {}
        }

        if let Some(peer_remote_addr) = network.get_remote_listen_addr(&peer_addr) {
            info!(
                "({}) handle hello request,save peers {}",
                config.local_addr, peer_remote_addr
            );
            let mut update_values: Peer = Peer::new();
            update_values.set_num_failures(0);
            update_values.set_active_time(chrono::Local::now().timestamp_millis());
            update_values.set_next_attempt_time(-1);
            update_values.set_address(peer_remote_addr.to_string());
            if !P2PStorage::update_item(network.peer_table(), &peer_remote_addr, update_values) {
                error!("failed to connect peer, update peers failed");
                return;
            }
        }
    }

    ///
    pub fn handle_hello_response(
        network: &PeerNetwork,
        peer_addr: SocketAddr,
        proto_message: &ProtocolsMessage,
    ) {
        let message: HelloResponseMessage =
            Message::parse_from_bytes(proto_message.get_data()).unwrap();
        if message.err_code != ErrCode::ERRCODE_SUCCESS {
            error!(
                "failed to response the peer hello message.Peer reponse error code {}, desc {}",
                message.get_err_code() as i64,
                message.get_err_desc()
            );
        }
    }
}

pub struct PeerSyncMessageHandler;

impl PeerSyncMessageHandler {
    ///
    pub async fn send_peersync_request(network: &PeerNetwork, peer_id: Endpoint) {
        let mut peers = Peers::default();
        if network.num_connected() < PEER_DB_COUNT {
            if network.conn_ids().contains(&peer_id) {
                if let Some(peer_remote_addr) = network.get_remote_listen_addr(&peer_id) {
                    P2PStorage::create_peer(
                        network,
                        network.peer_table(),
                        &mut peer_remote_addr.clone(),
                    );
                }
            }
        }

        P2PStorage::query_top_item(network.peer_table(), 50, &mut peers);
        let mut msg = ProtocolsMessage::new();
        msg.set_msg_type(ProtocolsMessageType::PEERS);
        msg.set_action(ProtocolsActionMessageType::REQUEST);
        msg.set_data(peers.write_to_bytes().unwrap());

        let peer_remote_addr = network.get_remote_listen_addr(&peer_id);
        let v: Vec<String> = peers
            .get_peers()
            .iter()
            .map(|x| x.get_address().to_string())
            .collect();
        info!(parent:network.span(),
            "({}) send peers({:?}) to({})--remote_addr({:?})",
            network.network_config().local_addr,
            v,
            peer_id,
            peer_remote_addr
        );
        network.send_msg(peer_id, msg);
    }

    ///
    pub fn handle_peersync_request(
        network: &PeerNetwork,
        peer_addr: Endpoint,
        proto_message: &ProtocolsMessage,
    ) {
        let message: Peers = Message::parse_from_bytes(proto_message.get_data()).unwrap();

        let peer_remote_addr = network.get_remote_listen_addr(&peer_addr);

        info!(parent:network.span(),
            "({}) handle peerssync,receive peers({:?}) from({})--remote_addr({:?})",
            network.network_config().local_addr,
            message.get_peers(),
            peer_addr,
            peer_remote_addr
        );

        for iter in message.peers {
            let ip_port_result = SocketAddr::from_str(iter.get_address());
            if ip_port_result.is_err() {
                continue;
            }

            let ip_port = ip_port_result.unwrap();

            if network.is_connected(ip_port) {
                info!(parent:network.span(),"skipped to connect the existed ip  {:?}", ip_port.clone());
                continue;
            }

            if ip_port == network.network_config().local_addr {
                trace!(parent:network.span(),"skipped to connect self ip {:?}", ip_port.clone());
                continue;
            }

            if network.num_connected() < PEER_DB_COUNT {
                let r = P2PStorage::create_peer(network, network.peer_table(), &ip_port.clone());
                info!(parent:network.span(),"handle peerssync,create_peer({}) result({})", ip_port, r);
            }
        }
    }
}

pub struct PingMessageHandler;

impl PingMessageHandler {
    pub fn send_ping_request(network: &PeerNetwork, peer_id: Endpoint) {
        let mut msg = ProtocolsMessage::new();
        msg.set_msg_type(ProtocolsMessageType::PING);
        msg.set_action(ProtocolsActionMessageType::REQUEST);

        trace!(parent:network.span(),"send ping request to({}--{:?})",peer_id.addr(),network.get_remote_address(&peer_id));
        network.send_msg(peer_id, msg);
    }

    pub fn handle_ping_request(network: &PeerNetwork, peer_id: Endpoint) {
        let mut msg = ProtocolsMessage::new();
        msg.set_msg_type(ProtocolsMessageType::PING);
        msg.set_action(ProtocolsActionMessageType::RESPONSE);

        trace!(parent:network.span(),"send ping response to({}--{:?})",peer_id.addr(),network.get_remote_address(&peer_id));
        network.send_msg(peer_id, msg);
    }
}
