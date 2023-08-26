use crate::utils::P2PUtils;
use configure::CONFIGURE_INSTANCE_REF;
use std::net::SocketAddr;
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;
use tracing::*;
use uuid::Uuid;

/// NetworkConfig for transaction or consensus msg broadcast
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum NetworkConfigType {
    /// for transaction broadcast
    Normal,
    /// for consensus broadcast
    Consensus,
}
impl ::std::fmt::Display for NetworkConfigType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub known_peers: Vec<String>,
    pub listen_addr: SocketAddr,
    pub local_addr: SocketAddr,
    pub target_conns_num: i64,
    pub node_address: String,
    pub node_rand: String,
    pub network_id: u64,
    pub chain_id: String,
    pub chain_hub: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            known_peers: Vec::new(),
            listen_addr: SocketAddr::new(IpAddr::from(Ipv4Addr::UNSPECIFIED), 0),
            local_addr: SocketAddr::new(IpAddr::from(Ipv4Addr::UNSPECIFIED), 0),
            target_conns_num: 0,
            node_address: Default::default(),
            node_rand: Default::default(),
            network_id: 0,
            chain_id: Default::default(),
            chain_hub: Default::default(),
        }
    }
}

impl NetworkConfig {
    pub fn new(config_type: NetworkConfigType) -> Self {
        let conf = CONFIGURE_INSTANCE_REF.clone();

        let mut listen_addr_string = String::from("");
        let mut known_peers: Vec<String> = Vec::new();
        if config_type == NetworkConfigType::Normal {
            listen_addr_string.clone_from(&conf.p2p_network.listen_addr);
            known_peers.clone_from(&conf.p2p_network.known_peers);
        } else {
            listen_addr_string.clone_from(&conf.p2p_network.consensus_listen_addr);
            known_peers.clone_from(&conf.p2p_network.consensus_known_peers);
        }

        let listen_addr_result = SocketAddr::from_str(listen_addr_string.as_str());
        if listen_addr_result.is_err() {
            info!(
                "listen_addr({:?}) config_type:({:?}) error ({:?})",
                listen_addr_string, config_type, listen_addr_result
            );
            std::process::exit(exitcode::CONFIG);
        }
        let listen_addr = listen_addr_result.unwrap();

        let mut local_ip = String::from("");

        let local_ip1 = P2PUtils::get_local_address();
        let local_ip2 = P2PUtils::get_local_addr();
        println!("local address({:?})--({:?})", local_ip1, local_ip2);

        if !conf.p2p_network.local_addr.is_empty() {
            let ips = P2PUtils::resolve_address(conf.p2p_network.local_addr.clone());
            let set: Vec<String> = ips.iter().map(|i| i.ip().to_string()).collect();
            // info!("local set-----------({:?})-----------------", set,);
            if let Some(local) = local_ip1 {
                if set.contains(&local) {
                    local_ip = local;
                }
            }
            if local_ip.is_empty() {
                if let Some(local) = local_ip2 {
                    if set.contains(&local) {
                        local_ip = local;
                    }
                }
            }
            if local_ip.is_empty() && !set.is_empty() {
                for x in set.iter() {
                    local_ip.clone_from(x);
                }
            }
        } else {
            if let Some(local) = local_ip1 {
                local_ip = local;
            }
            if local_ip.is_empty() {
                if let Some(local) = local_ip2 {
                    local_ip = local;
                }
            }
        }

        if local_ip.is_empty() {
            error!("local_ip({:?})", local_ip,);
            std::process::exit(exitcode::CONFIG);
        }

        let mut str_local_addr = String::default();
        str_local_addr.push_str(local_ip.as_str());
        str_local_addr.push_str(":");
        str_local_addr.push_str(listen_addr.port().to_string().as_str());
        let local_addr = SocketAddr::from_str(str_local_addr.as_str());

        if local_addr.is_err() {
            info!("local_addr error({:?})", local_addr.err());
            std::process::exit(exitcode::DATAERR);
        }
        let local_addr = local_addr.unwrap();

        info!("select local address({:?})", local_addr,);

        Self {
            known_peers: known_peers,
            listen_addr: listen_addr,
            local_addr: local_addr,
            node_address: conf.node_address.clone(),
            node_rand: Uuid::new_v4().to_string(),
            network_id: conf.network_id,
            chain_id: conf.chain_id.clone(),
            chain_hub: conf.chain_hub.clone(),
            target_conns_num: conf.p2p_network.target_peer_connection as i64,
        }
    }
}
