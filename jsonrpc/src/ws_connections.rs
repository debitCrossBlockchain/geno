use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    ops::Deref,
    sync::Arc,
};
use tokio::sync::mpsc;
use utils::{general::hash_crypto_byte, proto2json::proto_to_json};
use warp::{ws::Message, Filter, Rejection};

use crate::errors::JsonRpcError;

pub const TOPIC_TRANSACTIONS: &str = "transactions";
pub const TOPIC_HEADERS: &str = "headers";
pub const TOPIC_LOGS: &str = "logs";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubscribeLogs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Vec<String>>,
    pub topics: Vec<String>,
}

impl SubscribeLogs {
    pub fn into_bytes(&self) -> Vec<u8> {
        let mut value = TOPIC_LOGS.to_string();
        if let Some(addres) = &self.address {
            value.clone_from(&addres.join("-"));
        }
        if self.topics.len() > 0 {
            let r = self.topics.join("-");
            value += &r;
        }

        value.as_bytes().to_vec()
    }
}

#[derive(Debug, Clone)]
pub enum SubscribeTopic {
    Transactions(String),
    Headers(String),
    Logs((String, SubscribeLogs)),
}

impl SubscribeTopic {
    pub fn subscribe_id(&self) -> String {
        match self {
            SubscribeTopic::Transactions(v) => hex::encode(hash_crypto_byte(v.as_bytes())),
            SubscribeTopic::Headers(v) => hex::encode(hash_crypto_byte(v.as_bytes())),
            SubscribeTopic::Logs(v) => hex::encode(hash_crypto_byte(&v.1.into_bytes())),
        }
    }
}

pub enum PublishEvent {
    Transactions(protos::ledger::TransactionSignStore),
    Headers(protos::ledger::LedgerHeader),
    Logs(protos::common::ContractEvent),
}

impl PublishEvent {
    pub fn contain(&self, topic: &SubscribeTopic) -> Option<&dyn protobuf::Message> {
        match self {
            PublishEvent::Transactions(value) => match topic {
                SubscribeTopic::Transactions(_) => return Some(value),
                _ => {
                    return None;
                }
            },
            PublishEvent::Headers(value) => match topic {
                SubscribeTopic::Headers(_) => return Some(value),
                _ => {
                    return None;
                }
            },
            PublishEvent::Logs(value) => match topic {
                SubscribeTopic::Logs((_, log)) => {
                    if let Some(addres) = &log.address {
                        if addres.contains(&value.get_address().to_string()) {
                            return Some(value);
                        }
                    }
                    for t in &log.topics {
                        if value.get_topic().contains(t) {
                            return Some(value);
                        }
                    }
                    return None;
                }
                _ => {
                    return None;
                }
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct WsConnection {
    pub id: String,
    // subscribe id : subscribe content
    pub topics: HashMap<String, SubscribeTopic>,
    pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>,
}

pub struct InnerConnections {
    pub connections: HashMap<String, WsConnection>,
}

#[derive(Clone)]
pub struct WsConnections {
    pub conns: Arc<RwLock<InnerConnections>>,
    pub chain_id: String,
}

impl Deref for WsConnections {
    type Target = Arc<RwLock<InnerConnections>>;
    fn deref(&self) -> &Self::Target {
        &self.conns
    }
}

impl WsConnections {
    pub fn new(chain_id: String) -> Self {
        Self {
            conns: Arc::new(RwLock::new(InnerConnections {
                connections: HashMap::new(),
            })),
            chain_id,
        }
    }

    pub fn chain_id(&self) -> String {
        self.chain_id.clone()
    }

    pub fn contains(&self, id: &str) -> bool {
        self.conns.read().connections.contains_key(id)
    }

    pub fn set(&self, id: String, connection: WsConnection) -> bool {
        match self.conns.write().connections.entry(id) {
            std::collections::hash_map::Entry::Occupied(_) => {
                return false;
            }
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(connection);
                return true;
            }
        }
    }

    pub fn del(&self, id: &str) {
        self.conns.write().connections.remove(id);
    }

    pub fn add_topic(
        &self,
        id: &str,
        params: &Vec<serde_json::Value>,
    ) -> Result<String, JsonRpcError> {
        if let Some(conn) = self.conns.write().connections.get_mut(id) {
            let topic = Self::parse_params(params)?;
            let subscribe_id = topic.subscribe_id();
            conn.topics.insert(topic.subscribe_id(), topic);
            return Ok(subscribe_id);
        }
        Err(JsonRpcError::internal_error(
            "connection not exist".to_string(),
        ))
    }
    pub fn del_topic(&self, id: &str, subscribe_id: &str) {
        if let Some(conn) = self.conns.write().connections.get_mut(id) {
            conn.topics.remove(subscribe_id);
        }
    }

    pub fn send_to(&self, id: &str, message: Message) {
        if let Some(conn) = self.conns.read().connections.get(id) {
            if let Some(sender) = &conn.sender {
                if let Err(e) = sender.send(Ok(message)) {
                    eprintln!("send to client error: {}", e);
                }
            }
        }
    }

    pub fn publish_event(&self, event: PublishEvent) {
        for (_, conn) in self.conns.read().connections.iter() {
            for (_, topic) in conn.topics.iter() {
                if let Some(value) = event.contain(topic) {
                    if let Some(sender) = &conn.sender {
                        let john = proto_to_json(value);
                        if let Err(e) = sender.send(Ok(Message::text(john.to_string()))) {
                            eprintln!("send to client error: {}", e);
                        }
                    }
                }
            }
        }
    }

    fn parse_params(params: &Vec<serde_json::Value>) -> Result<SubscribeTopic, JsonRpcError> {
        if let Some(value) = params.get(0) {
            if let Some(sub_type) = value.as_str() {
                if sub_type == TOPIC_TRANSACTIONS {
                    if params.len() != 1 {
                        return Err(Self::param_err("ws subscribe", "params size error"));
                    }
                    return Ok(SubscribeTopic::Transactions(TOPIC_TRANSACTIONS.to_string()));
                } else if sub_type == TOPIC_HEADERS {
                    if params.len() != 1 {
                        return Err(Self::param_err("ws subscribe", "params size error"));
                    }
                    return Ok(SubscribeTopic::Transactions(TOPIC_HEADERS.to_string()));
                } else if sub_type == TOPIC_LOGS {
                    if params.len() != 2 {
                        return Err(Self::param_err("ws subscribe", "params size error"));
                    }

                    let value = params.get(1).unwrap();
                    let logs: SubscribeLogs = match serde_json::from_str(value.to_string().as_str())
                    {
                        Ok(logs) => logs,
                        Err(e) => {
                            return Err(Self::param_err("ws subscribe", "params logs format"));
                        }
                    };

                    return Ok(SubscribeTopic::Logs((TOPIC_LOGS.to_string(), logs)));
                } else {
                    return Err(Self::param_err(
                        "ws subscribe",
                        "params subscribe type exception",
                    ));
                }
            } else {
                return Err(Self::param_err(
                    "ws subscribe",
                    "params subscribe type null",
                ));
            }
        }
        return Err(Self::param_err("ws subscribe", "params size error"));
    }

    fn param_err(name: &str, err_msg: &str) -> JsonRpcError {
        JsonRpcError::invalid_param("ws subscribe", "params size error")
    }
}

pub async fn process_publish_event(
    connections: WsConnections,
    mut event_receiver: mpsc::UnboundedReceiver<PublishEvent>,
) {
    while let Some(event) = event_receiver.recv().await {
        connections.publish_event(event);
    }
}
