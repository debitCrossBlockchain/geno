use crate::connections::ConnectionSide;
use crate::know_broadcast::{BroadcastRecord, KnownBroadcasts};
use crate::message_handler::echo_message_handler::{HelloMessageHandler, PingMessageHandler};
use crate::message_handler::{
    ProtocolMessageHandler, ProtocolsMessageHandler, ReturnableProtocolsMessage,
};
use crate::network_config::{NetworkConfig, NetworkConfigType};
use crate::storage::P2PStorage;
use crate::utils::{P2PUtils, P2P_LIMIT_SIZE};
use crate::{utils, LocalBusBuilder, LocalBusPublisher, LocalBusSubscriber, SendMessageType};
use crate::{Node, Peer2Peer};
use anyhow::Result;
use crossbeam_channel::{after, select, tick};
use crossbeam_channel::{bounded, Receiver, Sender};
use message_io::network::{Endpoint, NetEvent, SendStatus, Transport};
use message_io::util::thread::NamespacedThread;
use parking_lot::{Mutex, RwLock};
use protobuf::wire_format::WireType::WireTypeLengthDelimited;
use protobuf::Message;
use protos::common::{
    Peer, Peers, ProtocolsActionMessageType, ProtocolsMessage, ProtocolsMessageType,
};
use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use std::net::SocketAddr;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::SeqCst};
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tracing::*;

pub const TEST_PERFORMANCE: bool = false;

pub struct InnerPeerNetwork {
    node: Node,
    sequence: AtomicU64,
    protocols: ProtocolsMessageHandler,
    tasks: Mutex<Vec<NamespacedThread<()>>>,
    reader_counters: Mutex<HashMap<Endpoint, u64>>,
    writer_msg_sender: Sender<SendMessageType>,
    writer_msg_recver: Receiver<SendMessageType>,
    bus: LocalBusBuilder<ProtocolsMessageType, ReturnableProtocolsMessage>,
    peer_table: String,
    network_config: NetworkConfig,
    known_broadcasts: KnownBroadcasts,
    validators: RwLock<HashSet<String>>,
    addresses_inited: AtomicBool,
}

/// PeerNetwork
#[derive(Clone)]
pub struct PeerNetwork(Arc<InnerPeerNetwork>);

impl Deref for PeerNetwork {
    type Target = Arc<InnerPeerNetwork>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Peer2Peer for PeerNetwork {
    fn node(&self) -> &Node {
        &self.node
    }
}

impl PeerNetwork {
    // no have active link config
    pub fn start(
        listen_addr: &str,
        name: &str,
        peer_table: &str,
        config_type: NetworkConfigType,
    ) -> PeerNetwork {
        let network_config = NetworkConfig::new(config_type);

        let (writer_msg_sender, writer_msg_recver) = bounded(1024);
        let network = PeerNetwork {
            0: Arc::new(InnerPeerNetwork {
                node: Node::new(listen_addr, name, config_type),
                sequence: Default::default(),
                protocols: Default::default(),
                tasks: Default::default(),
                reader_counters: Default::default(),
                writer_msg_sender,
                writer_msg_recver,
                bus: LocalBusBuilder::<ProtocolsMessageType, ReturnableProtocolsMessage>::new(),
                peer_table: peer_table.to_string(),
                network_config,
                known_broadcasts: Default::default(),
                validators: RwLock::new(HashSet::new()),
                addresses_inited: AtomicBool::new(false),
            }),
        };

        network.enable_reading_data();
        network.enable_writing_msg();
        network.enable_timer();
        network
    }

    pub fn start_service(peer_table: &str, config_type: NetworkConfigType) -> PeerNetwork {
        let network_config = NetworkConfig::new(config_type);

        let node_address = network_config.node_address.clone();
        let name = node_address
            .as_str()
            .chars()
            .skip(node_address.len() - 4)
            .take(4)
            .collect::<String>();
        let (writer_msg_sender, writer_msg_recver) = bounded(1024);
        let network = PeerNetwork {
            0: Arc::new(InnerPeerNetwork {
                node: Node::new(&network_config.listen_addr.to_string(), &name, config_type),
                sequence: Default::default(),
                protocols: Default::default(),
                tasks: Default::default(),
                reader_counters: Default::default(),
                writer_msg_sender,
                writer_msg_recver,
                bus: LocalBusBuilder::<ProtocolsMessageType, ReturnableProtocolsMessage>::new(),
                peer_table: peer_table.to_string(),
                network_config,
                known_broadcasts: Default::default(),
                validators: RwLock::new(HashSet::new()),
                addresses_inited: AtomicBool::new(false),
            }),
        };

        P2PStorage::reset_peer_active(&network, &network.peer_table);
        P2PStorage::resolve_peer(
            &network,
            &network.peer_table,
            &network.network_config.known_peers,
        );

        network.enable_reading_data();
        network.enable_writing_msg();
        network.enable_timer();
        network.enable_echo_msg_handler();
        network
    }

    pub fn span(&self) -> &tracing::Span {
        &self.node().span()
    }

    pub fn connect(&self, server_addr: &str) -> Result<(Endpoint, SocketAddr)> {
        self.node().connect(server_addr)
    }

    /// Checks whether the provided address is connected.
    pub fn is_connected(&self, address: SocketAddr) -> bool {
        self.node().connections.is_connected(address)
    }

    pub fn conn_side(&self, endpoint: &Endpoint) -> ConnectionSide {
        self.node().conn_side(endpoint)
    }

    pub fn num_connected(&self) -> usize {
        self.node().num_connected()
    }

    pub fn send_data(&self, endpoint: Endpoint, data: &[u8]) -> bool {
        // info!(parent:self.node().span(),"send to {}", endpoint);
        let r = self.node().send(endpoint, data);
        if r != SendStatus::Sent {
            error!(parent:self.node().span(),"send to {} error {:?}" ,endpoint,r);
            return false;
        }
        true
    }

    pub fn send_msg_raw(&self, endpoint: Endpoint, msg: ProtocolsMessage) -> bool {
        let data = msg.write_to_bytes().unwrap();
        self.send_data(endpoint, &data)
    }

    pub fn broadcast_msg_raw(&self, mut msg: ProtocolsMessage) {
        msg.mut_route()
            .push(self.network_config().listen_addr.to_string());

        if !P2PUtils::msg_has_hash(&msg) {
            let hash = P2PUtils::generate_hash(
                &self.node().name,
                self.node().config_type as i32,
                msg.get_sequence(),
            );
            msg.set_hash(hash);
        }

        // if !P2PUtils::msg_has_hash(&msg) {
        //     let hash = P2PUtils::get_hash(msg.get_data());
        //     msg.set_hash(hash);
        // }

        let data = msg.write_to_bytes().unwrap();
        let hash = msg.get_hash();

        match self.in_queued(hash) {
            Some(mut except_peers) => {
                except_peers.insert(self.network_config.local_addr);

                let pairs = self.node().get_all_ready_pairs();
                let mut sended_peers = HashSet::default();
                for ((endpoint, remote_socket)) in pairs.iter() {
                    if !except_peers.contains(remote_socket)
                        && !sended_peers.contains(remote_socket)
                    {
                        self.send_data(endpoint.clone(), &data);
                        sended_peers.insert(remote_socket.clone());
                    }
                }
                self.known_broadcasts.add_set(&hash, sended_peers);
            }
            None => {
                let pairs = self.node().get_all_ready_pairs();
                let mut sended_peers = HashSet::default();
                for ((endpoint, remote_socket)) in pairs.iter() {
                    if !sended_peers.contains(remote_socket) {
                        self.send_data(endpoint.clone(), &data);
                        sended_peers.insert(remote_socket.clone());
                    }
                }
                let record = BroadcastRecord {
                    time_stamp: chrono::Local::now().timestamp_millis(),
                    peers: sended_peers,
                };
                self.known_broadcasts.insert(hash.to_string(), record);
            }
        }
    }

    fn enable_reading_data(&self) {
        let read_data_thread = {
            let self_clone = self.clone();
            let reader_recver = self.node().reader_data_recver.clone();
            NamespacedThread::spawn("read_data_processer", move || {
                let mut last_time = chrono::Local::now().timestamp_millis();
                let publisher = self_clone.bus.publisher();
                loop {
                    select! {
                        recv(reader_recver) -> msg =>{
                            match msg {
                                Ok((endpoint,data))=>{
                                    // if let Err(e) = self_clone.process_buffer(endpoint,data,&self_clone.node().reader_data_sender){
                                    //     error!(parent:self_clone.node().span(),"process recv data {}",e);
                                    // }

                                    match P2PUtils::proto_msg_deserialize(&data){
                                        Ok(message)=>{
                                            // ===============statistic==============================================
                                            if TEST_PERFORMANCE {
                                                let now = chrono::Local::now().timestamp_millis();
                                                let span = now - last_time;
                                                if span >= 1000 {
                                                    last_time = now;
                                                    info!(parent:self_clone.node().span(),"recv msg {} span {}", message.get_sequence(), span);
                                                }
                                                self_clone.reader_counters.lock().entry(endpoint).and_modify(|v| *v += 1).or_insert(1);
                                            }
                                            // ======================================================================

                                            self_clone.handle_message(endpoint,message,&publisher);
                                        }

                                        Err(e)=>{error!(parent:self_clone.node().span(),"parse error {}",e);}
                                    }

                                }
                                Err(e)=>{
                                    error!(parent:self_clone.node().span(),"process recv data {}",e);
                                }
                            }
                        }
                    }
                }
            })
        };
        self.tasks.lock().push(read_data_thread);
    }

    // convert data to frame
    // pub fn process_buffer(
    //     &self,
    //     endpoint: Endpoint,
    //     data: Vec<u8>,
    //     reader_msg_sender: &Sender<(Endpoint, Vec<u8>)>,
    // ) -> std::io::Result<()> {
    //     self.node.process_buffer(endpoint, data, reader_msg_sender)
    // }

    // process frame
    // pub fn start_reading_msg(&self) {
    //     let read_msg_thread = {
    //         let self_clone = self.clone();
    //         let reader_recver = self.node().reader_msg_recver.clone();
    //         NamespacedThread::spawn("read_msg_processer", move || loop {
    //             select! {
    //                 recv(reader_recver) -> msg =>{
    //                     match msg {
    //                         Ok((endpoint,data))=>{
    //                             self_clone.process_msg(endpoint,data);
    //                         }
    //                         Err(e)=>{
    //                             error!(parent:self_clone.node().span(),"process recv msg {}",e);
    //                         }
    //                     }
    //                 }
    //             }
    //         })
    //     };
    //     self.tasks.lock().push(read_msg_thread);
    // }

    // pub fn process_msg(&self, endpoint: Endpoint, data: Vec<u8>) {
    //     if let Some(v) = self.reader_counters.write().get_mut(&endpoint) {
    //         *v += 1;
    //     } else {
    //         self.reader_counters.write().insert(endpoint, 1);
    //     }
    // }

    fn enable_writing_msg(&self) {
        let write_msg_thread = {
            let self_clone = self.clone();
            let reader_recver = self.writer_msg_recver.clone();
            NamespacedThread::spawn("write_msg_processer", move || loop {
                select! {
                    recv(reader_recver) -> msg =>{
                        match msg {
                            Ok((op_endpoint,mut proto_msg))=>{
                                if proto_msg.get_sequence() == 0 {
                                    proto_msg.set_sequence(self_clone.sequence());
                                }
                                if let Some(endpoint) = op_endpoint{
                                    self_clone.send_msg_raw(endpoint, proto_msg);
                                }else{
                                    self_clone.broadcast_msg_raw(proto_msg);
                                }
                            }
                            Err(e)=>{
                                error!(parent:self_clone.node().span(),"process send msg {}",e);
                            }
                        }
                    }
                }
            })
        };
        self.tasks.lock().push(write_msg_thread);
    }

    fn enable_timer(&self) {
        let timer_thread = {
            let self_clone = self.clone();
            NamespacedThread::spawn("write_msg_processer", move || loop {
                self_clone.check_broadcast();
                self_clone.timer_connect_to_peers();
                sleep(std::time::Duration::from_secs(5));
                // self_clone.check_connection_timeout();
            })
        };
        self.tasks.lock().push(timer_thread);
    }

    // async fn enable_echo_msg_handler(&self) {
    //     let (sender, mut receiver) =
    //         tokio::sync::mpsc::unbounded_channel::<ReturnableProtocolsMessage>();
    //     let _ = self.protocols.echo_message_handler.set(sender.into());
    //     let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    //     let self_clone = self.clone();
    //     let echo_task = tokio::spawn(async move {
    //         tx.send(()).unwrap(); // safe; the channel was just opened

    //         while let Some((peer_endpoint, proto_message)) = receiver.recv().await {
    //             match proto_message.get_msg_type() {
    //                 ProtocolsMessageType::PING => {
    //                     if proto_message.get_request() {
    //                         PingMessageHandler::handle_ping_request(&self_clone, peer_endpoint);
    //                     } else {
    //                     }
    //                 }
    //                 ProtocolsMessageType::HELLO => {
    //                     if proto_message.get_request() {
    //                         HelloMessageHandler::handle_hello_request(
    //                             &self_clone,
    //                             peer_endpoint,
    //                             &proto_message,
    //                         );
    //                     } else {
    //                     }
    //                 }
    //                 ProtocolsMessageType::PEERS => {
    //                     if proto_message.get_request() {
    //                     } else {
    //                     }
    //                 }
    //                 _ => {}
    //             }
    //         }
    //     });
    //     let _ = rx.await;
    //     self.node().async_tasks.lock().push(echo_task);
    // }

    fn enable_echo_msg_handler(&self) {
        let (sender, mut receiver) = bounded(1024);
        let _ = self.protocols.echo_message_handler.set(sender.into());

        let echo_thread = {
            let self_clone = self.clone();
            NamespacedThread::spawn("echo_processer", move || loop {
                select! {
                    recv(receiver) -> msg =>{
                        match msg {
                            Ok((peer_endpoint,mut proto_message))=>{
                                match proto_message.get_msg_type() {
                                    ProtocolsMessageType::PING => {
                                        if proto_message.get_action() == ProtocolsActionMessageType::REQUEST {
                                            PingMessageHandler::handle_ping_request(&self_clone, peer_endpoint);
                                        } else {
                                        }
                                    }
                                    ProtocolsMessageType::HELLO => {
                                        if proto_message.get_action() == ProtocolsActionMessageType::REQUEST {
                                            HelloMessageHandler::handle_hello_request(
                                                &self_clone,
                                                peer_endpoint,
                                                &proto_message,
                                            );
                                        } else {
                                        }
                                    }
                                    ProtocolsMessageType::PEERS => {
                                        if proto_message.get_action() == ProtocolsActionMessageType::REQUEST {
                                        } else {
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            Err(e)=>{
                                error!(parent:self_clone.node().span(),"process send msg {}",e);
                            }
                        }
                    }
                }
            })
        };
        self.tasks.lock().push(echo_thread);
    }

    fn echo_handler(&self) -> Option<&ProtocolMessageHandler> {
        self.protocols.echo_message_handler.get()
    }

    fn handle_message(
        &self,
        peer_endpoint: Endpoint,
        message: ProtocolsMessage,
        publisher: &LocalBusPublisher<ProtocolsMessageType, ReturnableProtocolsMessage>,
    ) {
        match message.get_action() {
            ProtocolsActionMessageType::REQUEST | ProtocolsActionMessageType::RESPONSE => {
                match message.get_msg_type() {
                    ProtocolsMessageType::PING
                    | ProtocolsMessageType::HELLO
                    | ProtocolsMessageType::PEERS => {
                        let msg: ReturnableProtocolsMessage = (peer_endpoint, message);
                        if let Some(handle) = self.echo_handler() {
                            let _ = handle.send(msg);
                        }
                    }
                    _ => {
                        if let Some(handle) = self
                            .protocols
                            .req_resp_handler
                            .read()
                            .get(&message.get_msg_type())
                        {
                            let msg: ReturnableProtocolsMessage = (peer_endpoint, message);
                            let _ = handle.send(msg);
                        }
                    }
                }
            }
            ProtocolsActionMessageType::BROADCAST => {
                self.handle_broadcast(peer_endpoint, message, publisher);
            }
        }
    }

    fn handle_broadcast(
        &self,
        peer_endpoint: Endpoint,
        message: ProtocolsMessage,
        publisher: &LocalBusPublisher<ProtocolsMessageType, ReturnableProtocolsMessage>,
    ) {
        if message.compute_size() > P2P_LIMIT_SIZE {
            error!(parent: self.node().span(),
                "failed to process the peer msg, msg size {} is too large",
                message.compute_size()
            );
            return;
        }
        let hash = message.get_hash();

        if self.is_queued(hash) {
            return;
        }

        // prevent duplicate msg
        if let Some(remote_peer) = self.get_remote_listen_addr(&peer_endpoint) {
            self.receive_broadcast_msg(hash, remote_peer.clone());
            // if message.get_msg_type() != ProtocolsMessageType::TRANSACTION {
            self.broadcast_msg(message.clone());
            // }
        } else {
            error!(parent: self.node().span(),
                "handle_broadcast failed to get_remote_listen_addr {}",
                 peer_endpoint.addr()
            );
        }

        publisher.publish(message.get_msg_type(), (peer_endpoint, message));
    }

    pub fn peer_table(&self) -> &str {
        &self.peer_table
    }

    pub fn get_remote_listen_addr(&self, peer_endpoint: &Endpoint) -> Option<SocketAddr> {
        self.node().get_remote_address(peer_endpoint)
    }

    fn receive_broadcast_msg(&self, hash: &str, peer_id: SocketAddr) -> bool {
        self.known_broadcasts.add_one(hash, peer_id)
    }

    pub fn network_config(&self) -> NetworkConfig {
        self.network_config.clone()
    }

    pub fn get_endpoint_by_node_id(&self, node_id: String) -> Option<Endpoint> {
        self.node().get_endpoint_by_node_id(node_id)
    }

    pub fn set_remote_info(
        &self,
        peer_addr: &Endpoint,
        remote_listen_addr: SocketAddr,
        node_id: String,
    ) {
        let is_validator = self.validators.read().contains(&node_id);
        self.node()
            .set_remote_info(peer_addr, remote_listen_addr, node_id, is_validator);
    }

    fn check_broadcast(&self) {
        self.known_broadcasts.check_timeout();
    }

    fn timer_connect_to_peers(&self) {
        let self_clone = self.clone();
        if !self.addresses_inited.load(SeqCst) {
            P2PStorage::resolve_peer(
                &self,
                &self_clone.peer_table,
                &self_clone.network_config().known_peers,
            );
            self.addresses_inited.store(true, SeqCst);
        }

        let con_size: i64 = self_clone.node().num_connected().clone() as i64;
        if con_size < self_clone.network_config.target_conns_num {
            self.connect_to_peers(self_clone.network_config.target_conns_num - con_size);
        }
        P2PStorage::clean_not_active_peers(&self, &self_clone.peer_table);
    }

    fn connect_to_peers(&self, max: i64) -> bool {
        let mut records: Peers = Peers::new();
        P2PStorage::resolve_peer(&self, &self.peer_table, &self.network_config.known_peers);
        let row_count = P2PStorage::query_top_item(&self.peer_table, max, &mut records);
        if row_count < 0 {
            error!(parent: self.node().span(), "failed to query records from db");
            return false;
        };

        for iter in records.peers.iter() {
            let mut num_failures = iter.num_failures;
            let ip_port_result = SocketAddr::from_str(iter.get_address());
            if ip_port_result.is_err() {
                error!(parent: self.node().span(),
                    "connect_to_peers parse net address({}) error",
                    iter.get_address()
                );
                continue;
            }
            let ip_port = ip_port_result.unwrap();

            let remote_addr_set = self.node().get_all_remote_address();
            if remote_addr_set.contains(&ip_port) {
                continue;
            }

            if ip_port == self.network_config.local_addr {
                continue;
            }

            info!(parent: self.node().span(), "try connect to {:?}", ip_port.to_string());
            if self.node().is_connected(ip_port.clone()) {
                continue;
            }

            let mut skip_continue = false;

            match self.node().connect(&ip_port.to_string()) {
                Err(e) => {
                    error!(parent: self.node().span(), "connect ({:?}) error({:?})", ip_port, e);
                    continue;
                }
                Ok((endpoint, socket)) => {
                    let mut start = Instant::now();
                    let mut flag = true;

                    while flag {
                        sleep(std::time::Duration::from_millis(100));
                        if self.node().already_connected(&endpoint) {
                            //say hello
                            HelloMessageHandler::send_hello_request(self, endpoint);
                            flag = false;
                        }
                        if start.elapsed().as_millis() >= 1000 {
                            error!(parent: self.node().span(), "connect ({}) time out", endpoint.addr());
                            skip_continue = true;
                            flag = false;
                        }
                    }
                }
            }
            if skip_continue {
                continue;
            }

            let mut update_values: Peer = Peer::new();
            num_failures += 1;
            update_values.set_next_attempt_time(
                chrono::Local::now().timestamp_millis() + num_failures * 10 * 1000,
            );
            update_values.set_active_time(-1);
            update_values.set_num_failures(num_failures);
            update_values.set_address(ip_port.to_string());
            if !P2PStorage::update_item(&self.peer_table, &ip_port, update_values) {
                error!(parent: self.node().span(), "failed to connect peer, update peers failed");
            }

            if self.node().num_connected() >= self.network_config.target_conns_num as usize {
                break;
            }
        }
        true
    }

    pub fn is_queued(&self, hash: &str) -> bool {
        self.known_broadcasts.contains(hash)
    }

    pub fn in_queued(&self, hash: &str) -> Option<HashSet<SocketAddr>> {
        self.known_broadcasts.has(hash)
    }

    fn perform_handshake(&self, endpoint: Endpoint) {
        PingMessageHandler::send_ping_request(&self, endpoint);
    }

    fn check_connection_timeout(&self) {
        let pairs = self.node().get_all_ready_timeout();
        for ((endpoint, timeout)) in pairs.iter() {
            if timeout.elapsed().as_secs() >= 10 {
                self.perform_handshake(endpoint.clone());
            }
        }
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn get_remote_address(&self, endpoint: &Endpoint) -> Option<SocketAddr> {
        self.node().get_remote_address(endpoint)
    }

    pub fn listen_endpoint(&self) -> Endpoint {
        self.node().listen_endpoint
    }

    pub fn add_subscriber(
        &self,
        topic: ProtocolsMessageType,
    ) -> LocalBusSubscriber<ProtocolsMessageType, ReturnableProtocolsMessage> {
        self.bus.add_subscriber(&[topic])
    }

    pub fn publisher(&self) -> LocalBusPublisher<ProtocolsMessageType, ReturnableProtocolsMessage> {
        self.bus.publisher()
    }

    pub fn send_msg_to_peer(&self, node_id: String, msg: ProtocolsMessage) -> bool {
        if let Some(endpoint) = self.get_endpoint_by_node_id(node_id) {
            return self.send_msg(endpoint, msg);
        }
        false
    }

    pub fn send_msg(&self, endpoint: Endpoint, msg: ProtocolsMessage) -> bool {
        if let Err(e) = self.writer_msg_sender.try_send((Some(endpoint), msg)) {
            error!(parent:self.node().span(),"send msg error {}",e);
            return false;
        }
        true
    }

    pub fn broadcast_msg(&self, msg: ProtocolsMessage) -> bool {
        if let Err(e) = self.writer_msg_sender.try_send((None, msg)) {
            error!(parent:self.node().span(),"broadcast msg error {}",e);
            return false;
        }
        true
    }

    pub fn register_rpc_handler(
        &self,
        msg_type: ProtocolsMessageType,
        sender: Sender<(Endpoint, ProtocolsMessage)>,
    ) {
        self.protocols
            .req_resp_handler
            .write()
            .entry(msg_type)
            .or_insert(sender.into());
    }

    pub fn conn_ids(&self) -> HashSet<Endpoint> {
        let mut set = HashSet::default();
        for it in self.node().connected_endpoints() {
            set.insert(it);
        }
        set
    }

    pub fn validator_conn_ids(&self) -> HashMap<String, Endpoint> {
        self.node().validator_connected_endpoints()
    }

    pub fn update_validators(&self, validator: &[String]) {
        let hash_set: HashSet<String> = validator.iter().cloned().collect();
        let mut v = self.validators.write();
        v.clear();
        v.clone_from(&hash_set);
    }
}
