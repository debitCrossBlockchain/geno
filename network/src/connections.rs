use crate::utils::{self, P2PUtils};
use crossbeam_channel::{bounded, Receiver, Sender};
use message_io::network::Endpoint;
use parking_lot::RwLock;
use std::{
    collections::{HashMap, HashSet},
    io,
    net::SocketAddr,
    ops::Not,
    time::Instant,
};

/// Indicates who was the initiator and who was the responder when the connection was established.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionSide {
    /// The side that initiated the connection.
    Initiator,
    /// The sider that accepted the connection.
    Responder,
}

impl Not for ConnectionSide {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::Initiator => Self::Responder,
            Self::Responder => Self::Initiator,
        }
    }
}

#[derive(Clone)]
pub struct RemotePeerInfo {
    /// peer listen ip
    pub address: SocketAddr,
    /// node address
    pub node_id: String,
    /// is validator
    pub is_validator: bool,
}

pub struct Connection {
    endpoint: Endpoint,
    side: ConnectionSide,
    remote_info: Option<RemotePeerInfo>,
    read_buf: Vec<u8>,
    last_time: Instant,
}

impl Connection {
    /// Creates a [`Connection`] with placeholders for protocol-related objects.
    pub(crate) fn new(endpoint: Endpoint, side: ConnectionSide) -> Self {
        Self {
            endpoint,
            side,
            remote_info: None,
            read_buf: Vec::default(),
            last_time: Instant::now(),
        }
    }

    /// Returns the endpoint associated with the connection.
    pub fn endpoint(&self) -> Endpoint {
        self.endpoint.clone()
    }

    /// Returns the address associated with the connection.
    pub fn addr(&self) -> SocketAddr {
        self.endpoint.addr()
    }

    /// Returns `Initiator` if the associated peer initiated the connection
    /// and `Responder` if the connection request was initiated by the node.
    pub fn side(&self) -> ConnectionSide {
        self.side
    }

    pub fn remote_address(&self) -> Option<SocketAddr> {
        if let Some(ref info) = self.remote_info {
            return Some(info.address.clone());
        }
        None
    }

    pub fn set_remote_info(&mut self, address: SocketAddr, node_id: String, is_validator: bool) {
        if let Some(ref mut info) = self.remote_info {
            info.address = address;
            info.node_id = node_id;
            info.is_validator = is_validator;
        } else {
            self.remote_info = Some(RemotePeerInfo {
                address,
                node_id,
                is_validator,
            });
        }
    }

    pub fn update_time(&mut self) {
        self.last_time = Instant::now();
    }

    pub fn process_data(
        &mut self,
        mut data: Vec<u8>,
        reader_msg_sender: &Sender<(Endpoint, Vec<u8>)>,
    ) -> io::Result<()> {
        self.read_buf.append(&mut data);
        let mut left = self.read_buf.len();
        let mut buf_reader = std::io::Cursor::new(&self.read_buf[..left]);
        loop {
            // the position in the buffer before the message read attempt
            let initial_buf_pos = buf_reader.position() as usize;

            // try to read a single message from the buffer
            let read = P2PUtils::read_message(&mut buf_reader);

            // the position in the buffer after the read attempt
            let post_read_buf_pos = buf_reader.position() as usize;

            // register the number of bytes that were processed by the Reading::read_message call above
            let parse_size = post_read_buf_pos - initial_buf_pos;

            match read {
                // a full message was read successfully
                Ok(Some(msg)) => {
                    left -= parse_size;

                    // send the message for further processing
                    if let Err(e) = reader_msg_sender.try_send((self.endpoint, msg)) {
                        return Err(io::ErrorKind::OutOfMemory.into());
                    }

                    // if the read is exhausted, clear the read buffer and return
                    if left == 0 {
                        self.read_buf.clear();
                        return Ok(());
                    }
                }
                // the message in the buffer is incomplete
                Ok(None) => {
                    self.read_buf
                        .copy_within(initial_buf_pos..initial_buf_pos + left, 0);
                    self.read_buf.truncate(left);
                    return Ok(());
                }
                // an erroneous message (e.g. an unexpected zero-length payload)
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct Connections(RwLock<HashMap<Endpoint, Connection>>);

impl Connections {
    pub(crate) fn add(&self, conn: Connection) {
        self.0.write().insert(conn.endpoint(), conn);
    }

    pub(crate) fn remove(&self, endpoint: &Endpoint) -> Option<Connection> {
        self.0.write().remove(endpoint)
    }

    pub(crate) fn is_connected(&self, address: SocketAddr) -> bool {
        for (e, c) in self.0.read().iter() {
            if e.addr() == address {
                return true;
            }
            if let Some(addr) = c.remote_address() {
                if addr == address {
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn already_connected(&self, endpoint: &Endpoint) -> bool {
        self.0.read().contains_key(endpoint)
    }

    pub(crate) fn num_connected(&self) -> usize {
        self.0.read().len()
    }

    pub(crate) fn endpoints(&self) -> Vec<Endpoint> {
        self.0.read().keys().copied().collect()
    }

    pub(crate) fn validator_endpoints(&self) -> HashMap<String, Endpoint> {
        let mut map: HashMap<String, Endpoint> = HashMap::new();
        for (e, c) in self.0.read().iter() {
            if let Some(ref info) = c.remote_info {
                if info.is_validator {
                    map.insert(info.node_id.clone(), e.clone());
                }
            }
        }
        map
    }

    pub fn get_endpoint_by_node_id(&self, node_id: String) -> Option<Endpoint> {
        for (e, c) in self.0.read().iter() {
            if let Some(ref info) = c.remote_info {
                if info.node_id == node_id {
                    return Some(e.clone());
                }
            }
        }
        None
    }

    pub(crate) fn get_all_remote_address(&self) -> HashSet<SocketAddr> {
        let mut set = HashSet::default();
        for (e, c) in self.0.read().iter() {
            if let Some(addr) = c.remote_address() {
                set.insert(addr);
            }
        }
        set
    }

    pub(crate) fn get_all_ready_pairs(&self) -> HashSet<(Endpoint, SocketAddr)> {
        let mut set = HashSet::default();
        for (e, c) in self.0.read().iter() {
            if let Some(addr) = c.remote_address() {
                set.insert((e.clone(), addr));
            }
        }
        set
    }

    pub(crate) fn get_all_ready_timeout(&self) -> HashSet<(Endpoint, Instant)> {
        let mut set = HashSet::default();
        for (e, c) in self.0.read().iter() {
            if let Some(addr) = c.remote_address() {
                set.insert((e.clone(), c.last_time.clone()));
            }
        }
        set
    }

    pub(crate) fn get_remote_address(&self, endpoint: &Endpoint) -> Option<SocketAddr> {
        if let Some(c) = self.0.read().get(endpoint) {
            if let Some(ref info) = c.remote_info {
                return Some(info.address.clone());
            }
        }
        None
    }

    pub(crate) fn conn_side(&self, endpoint: &Endpoint) -> ConnectionSide {
        let mut side = ConnectionSide::Initiator;
        if let Some(conn) = self.0.read().get(endpoint) {
            side.clone_from(&conn.side);
        };
        side
    }

    pub fn set_remote_info(
        &self,
        endpoint: &Endpoint,
        address: SocketAddr,
        node_id: String,
        is_validator: bool,
    ) {
        if let Some(c) = self.0.write().get_mut(endpoint) {
            c.set_remote_info(address, node_id, is_validator);
        }
    }

    pub fn update_time(&self, endpoint: &Endpoint) {
        if let Some(c) = self.0.write().get_mut(endpoint) {
            c.update_time();
        }
    }

    // convert data to frame
    // pub(crate) fn process_data(
    //     &self,
    //     endpoint: &Endpoint,
    //     data: Vec<u8>,
    //     reader_msg_sender: &Sender<(Endpoint, Vec<u8>)>,
    // ) -> io::Result<()> {
    //     if let Some(conn) = self.0.write().get_mut(&endpoint) {
    //         return conn.process_data(data, reader_msg_sender);
    //     }
    //     Err(io::ErrorKind::NotFound.into())
    // }
}
