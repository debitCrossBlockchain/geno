use std::net::SocketAddr;
use std::str::FromStr;
use thiserror::Error;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct Websocket{
    pub address: SocketAddr,
}


pub const DEFAULT_WEBSOCKET_ADDRESS: &str = "0.0.0.0";
pub const DEFAULT_WEBSOCKET_PORT: u16 = 8081;


impl Clone for Websocket{
    fn clone(&self) -> Self {
        Self{
            address: self.address,
        }
    }
}
//todo

impl Default for Websocket {
    fn default() -> Websocket {
        Websocket {
            address: format!("{}:{}", DEFAULT_WEBSOCKET_ADDRESS, DEFAULT_WEBSOCKET_PORT)
                .parse()
                .unwrap(),

        }
    }
}