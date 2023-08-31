use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Deserialize, Debug)]
pub struct WebsocketConfig {
    pub address: SocketAddr,
}

pub const DEFAULT_WEBSOCKET_ADDRESS: &str = "0.0.0.0";
pub const DEFAULT_WEBSOCKET_PORT: u16 = 8081;

impl Clone for WebsocketConfig {
    fn clone(&self) -> Self {
        Self {
            address: self.address,
        }
    }
}

impl Default for WebsocketConfig {
    fn default() -> WebsocketConfig {
        WebsocketConfig {
            address: format!("{}:{}", DEFAULT_WEBSOCKET_ADDRESS, DEFAULT_WEBSOCKET_PORT)
                .parse()
                .unwrap(),
        }
    }
}
