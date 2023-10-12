use crate::{
    errors::JsonRpcError,
    request::JsonRpcRequest,
    response::JsonRpcResponse,
    ws_connections::{WsConnection, WsConnections},
};
use futures::{Future, FutureExt, StreamExt};
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

use warp::{
    filters::ws::{Message, WebSocket},
    http::StatusCode,
    reply::json,
    Reply,
};

pub const METHOD_SUBSCRIBE: &str = "subscribe";
pub const METHOD_UNSUBSCRIBE: &str = "unsubscribe";

pub async fn ws_handler(
    websocket: WebSocket,
    remote: Option<SocketAddr>,
    api_key: String,
    connections: WsConnections,
) {
    println!(
        "ws_handler start process,api_key:{} remote:{:?}",
        api_key, remote
    );
    let (client_ws_sender, mut client_ws_rcv) = websocket.split();
    let (client_sender, client_rcv) = mpsc::unbounded_channel();

    let client_rcv = UnboundedReceiverStream::new(client_rcv);
    tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
        if let Err(e) = result {
            eprintln!("error sending websocket msg: {}", e);
        }
    }));

    let addr = match remote {
        Some(addr) => addr,
        None => {
            eprintln!("connection socket address is none");
            return;
        }
    };
    let addr_id = addr.to_string();
    let c = WsConnection {
        id: addr_id.clone(),
        topics: HashMap::new(),
        sender: Some(client_sender),
    };
    if !connections.set(addr_id.clone(), c) {
        eprintln!("connection socket address duplicate");
        return;
    }

    println!("connection {} enter loop recv", addr_id);
    while let Some(result) = client_ws_rcv.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!(
                    "error receiving ws message for id: {}): {}",
                    addr_id.clone(),
                    e
                );
                break;
            }
        };
        handle_websocket_message(&addr_id, msg, connections.clone());
    }

    connections.del(&addr_id);
    println!("connection {} disconnected", addr_id);
}

fn handle_websocket_message(conn_id: &str, msg: Message, connections: WsConnections) {
    println!(
        "handle_websocket_message received message from {}: {:?}",
        conn_id, msg
    );
    if msg.is_text() {
        handle_text_message(conn_id, msg, connections);
    } else {
        handle_bytes_message(conn_id, msg, connections);
    }
}

fn handle_text_message(conn_id: &str, msg: Message, connections: WsConnections) {
    let mut response = JsonRpcResponse::new(connections.chain_id());

    let message = match msg.to_str() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("error while message convert to str");
            response.error = Some(JsonRpcError::internal_error(
                "ws message convert to str".to_string(),
            ));
            send_text_response(conn_id, response, connections);
            return;
        }
    };
    if message == "ping" || message == "ping\n" {
        return;
    }

    let request: JsonRpcRequest = match serde_json::from_str(&message) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error while parsing message to request: {}", e);
            response.error = Some(JsonRpcError::internal_error(
                "parse json message error".to_string(),
            ));
            send_text_response(conn_id, response, connections);
            return;
        }
    };
    response.id = Some(serde_json::Value::Number(request.id.into()));

    if request.method == METHOD_SUBSCRIBE {
        match connections.add_topic(conn_id, &request.params) {
            Ok(subscribe_id) => {
                response.result = Some(serde_json::Value::String(subscribe_id));
                response.error = Some(JsonRpcError::no_error());
            }
            Err(e) => {
                response.error = Some(e);
                send_text_response(conn_id, response, connections.clone());
                return;
            }
        }
    } else if request.method == METHOD_SUBSCRIBE {
        for value in request.params.iter() {
            if let Some(subscribe_id) = value.as_str() {
                connections.del_topic(conn_id, subscribe_id);
            }
        }
    } else {
        response.error = Some(JsonRpcError::method_not_found());
        send_text_response(conn_id, response, connections);
        return;
    }

    send_text_response(conn_id, response, connections);
}

fn send_text_response(conn_id: &str, response: JsonRpcResponse, connections: WsConnections) {
    match serde_json::to_string(&response) {
        Ok(js) => connections.send_to(conn_id, Message::text(js)),
        Err(e) => {
            eprintln!("error while message convert to str");
        }
    }
}

fn handle_bytes_message(conn_id: &str, msg: Message, connections: WsConnections) {
    if msg.is_ping() || msg.is_pong() {
        return;
    }
}
