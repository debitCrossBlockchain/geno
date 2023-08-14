use message_io::network::{Endpoint, NetEvent, Transport, SendStatus, ResourceType, ResourceId};
use message_io::node::{self, NodeHandler, NodeListener, NodeTask};
use message_io::util::thread::NamespacedThread;
use parking_lot::{Mutex, RwLock};
use std::collections::{HashSet, HashMap};
use std::error;
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Instant;
use tracing::*;
use anyhow::Result;
use crossbeam_channel::{bounded,Sender,Receiver};

use crate::NetworkConfigType;
use crate::connections::{Connections, Connection, ConnectionSide};
use crate::message_handler::echo_message_handler::HelloMessageHandler;

static SEQUENTIAL_NODE_ID: AtomicUsize = AtomicUsize::new(0);

pub type Message = Vec<u8>;


#[derive(Clone)]
// #[cfg(target_os = "linux")]
pub struct Node(Arc<InnerNode>);

impl Deref for Node {
    type Target = Arc<InnerNode>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct InnerNode {
    pub(crate) name: String,
    pub(crate) config_type: NetworkConfigType,
    pub(crate) span: tracing::Span,
    pub(crate) tasks: Mutex<Vec<NodeTask>>,
    pub(crate) async_tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    pub(crate) handler: NodeHandler<()>,
    pub(crate) connections: Connections,
    pub(crate) reader_data_sender: Sender<(Endpoint,Vec<u8>)>,
    pub(crate) reader_data_recver: Receiver<(Endpoint,Vec<u8>)>,
    pub(crate) reader_msg_sender: Sender<(Endpoint,Vec<u8>)>,
    pub(crate) reader_msg_recver: Receiver<(Endpoint,Vec<u8>)>,
    pub(crate) listen_endpoint:Endpoint,
}



impl Node {
    pub fn new(listen_addr: &str, name: &str,config_type: NetworkConfigType) -> Node {
        let span = create_span(name);

        let (handler, node_listener) = node::split::<()>();
        match handler.network().listen(Transport::FramedTcp, listen_addr) {
            Ok((id, addr)) => {
                info!(parent: &span, "start listen {} {}", id, addr);
            }
            Err(e) => {
                error!(parent: &span, "start listen error {} ", e);
                std::process::exit(-1);
            }
        };

        let listen_endpoint  = Self::generate_listen_endpoint(listen_addr);

        let (reader_data_sender,reader_data_recver) = bounded(1024);
        let (reader_msg_sender,reader_msg_recver) = bounded(1024);
        let node = Node(Arc::new(InnerNode {
            name:name.to_string(),
            config_type,
            span,
            handler,
            tasks: Default::default(),
            async_tasks:Default::default(),
            connections: Default::default(),
            reader_data_sender,
            reader_data_recver,
            reader_msg_sender,
            reader_msg_recver,
            listen_endpoint
        }));

        let node_clone = node.clone();
        let task = node_listener.for_each_async(move |event| match event.network() {
            NetEvent::Connected(endpoint, established) => {
                info!(parent:node_clone.span(),"Event Connected endpoint({}--{} {}) {}",endpoint.addr(),endpoint.resource_id(),endpoint.resource_id().raw(),established);
                node_clone.connected_proc(endpoint,established);  
            }
            NetEvent::Accepted(endpoint, id) => {
                info!(parent:node_clone.span(),"Event Accepted endpoint({}--{} {}) resource_id({})",endpoint.addr(),endpoint.resource_id(),endpoint.resource_id().raw(),id);
                node_clone.accept_proc(endpoint);                
            }
            NetEvent::Message(endpoint, _data) => {
                // info!(parent:node_clone.span(),"Event Message {} ",endpoint.addr());
                node_clone.message_proc(endpoint,_data); 
            }
            NetEvent::Disconnected(endpoint) => {
                info!(parent:node_clone.span(),"Event Disconnected {} ",endpoint.addr());
                node_clone.disconnected_proc(endpoint); 
            }
        });
        node.tasks.lock().push(task);
        node
    }

    /// Returns the tracing [`Span`] associated with the node.
    #[inline]
    pub fn span(&self) -> &tracing::Span {
        &self.span
    }

    pub fn connect(&self, server_addr: &str) -> Result<(Endpoint, SocketAddr)> {
        match self
            .handler
            .network()
            .connect(Transport::FramedTcp, server_addr)
        {
            Ok((server_endpoint, client_address)) => {
                info!(parent:self.span(),"connect to {} result {} -- {}",server_addr,server_endpoint.addr(),client_address);
                return Ok((server_endpoint, client_address));                
            }
            Err(e) => {
                error!(parent:self.span(),"connect to {} error {}",server_addr,e);
                return Err(anyhow::anyhow!("connect to {} error {}",server_addr,e));
            }
        }
    }

    pub fn send(&self,endpoint: Endpoint,data:&[u8])->SendStatus{
        self.handler.network().send(endpoint,data)
    }

    /// Checks whether the provided address is connected.
    pub fn is_connected(&self, address: SocketAddr) -> bool {
        self.connections.is_connected(address)
    }

    pub fn conn_side(&self, endpoint:&Endpoint) -> ConnectionSide {
        self.connections.conn_side(endpoint)
    }

    pub fn connected_endpoints(&self) -> Vec<Endpoint> {
        self.connections.endpoints()
    }

    pub fn validator_connected_endpoints(&self)-> HashMap<String,Endpoint>{
        self.connections.validator_endpoints()
    }

    pub fn get_endpoint_by_node_id(&self, node_id: String) -> Option<Endpoint> {
        self.connections.get_endpoint_by_node_id(node_id)
    }

    pub fn already_connected(&self, endpoint: &Endpoint) -> bool {
        self.connections.already_connected(endpoint)
    }

    pub fn num_connected(&self) -> usize {
        self.connections.num_connected()
    }

    pub fn connected_proc(&self, endpoint: Endpoint,established:bool) {
        if established {
            let conn = Connection::new(endpoint,ConnectionSide::Responder);
            self.connections.add(conn);
        } else {
            error!(parent:self.span(),"connect to {} failed",endpoint);
        }
    }

    pub fn accept_proc(&self, endpoint: Endpoint) {
        let conn = Connection::new(endpoint,ConnectionSide::Initiator);
        self.connections.add(conn);
    }

    pub fn disconnected_proc(&self, endpoint: Endpoint) {
        self.connections.remove(&endpoint);
    }

    pub fn message_proc(&self, endpoint: Endpoint,data:&[u8]) {
        // self.connections.update_time(&endpoint); // 
        if let Err(e)= self.reader_data_sender.try_send((endpoint,data.to_vec())){
            error!(parent:self.span(),"reader_sender.send error({:?})",e);
        }
    }

     pub fn get_all_remote_address(&self) -> HashSet<SocketAddr> {
        self.connections.get_all_remote_address()
     }

     pub fn get_remote_address(&self, endpoint: &Endpoint) -> Option<SocketAddr> {
        self.connections.get_remote_address(endpoint)
     }

     pub fn set_remote_info(
        &self,
        endpoint: &Endpoint,
        address: SocketAddr,
        node_id: String,
        is_validator: bool,
    ){
        self.connections.set_remote_info(endpoint,address,node_id,is_validator);
    }

    pub fn get_all_ready_pairs(&self) -> HashSet<(Endpoint, SocketAddr)>{
        self.connections.get_all_ready_pairs()
    }

    pub fn get_all_ready_timeout(&self)-> HashSet<(Endpoint, Instant)>{
        self.connections.get_all_ready_timeout()
    }

    // pub fn process_buffer(&self, endpoint: Endpoint, data: Vec<u8>)-> io::Result<()> {
    //     self.connections.process_data(endpoint,data)
    // }

    pub fn generate_listen_endpoint(listen_addr:&str)->Endpoint{
        let (handler, listener) = node::split::<()>();
        let (sender_id, addr_1) = handler.network().listen(Transport::Udp, listen_addr).unwrap();
        let endpoint = Endpoint::from_listener(sender_id, addr_1);
        handler.stop();
        endpoint
    }
    
}

fn create_span(node_name: &str) -> Span {
    let mut span = trace_span!("node", name = node_name);
    if !span.is_disabled() {
        return span;
    } else {
        span = debug_span!("node", name = node_name);
    }
    if !span.is_disabled() {
        return span;
    } else {
        span = info_span!("node", name = node_name);
    }
    if !span.is_disabled() {
        return span;
    } else {
        span = warn_span!("node", name = node_name);
    }
    if !span.is_disabled() {
        span
    } else {
        error_span!("node", name = node_name)
    }
}
