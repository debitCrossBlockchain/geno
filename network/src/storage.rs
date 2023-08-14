use protobuf::Message;
use protos::common::*;
use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::str::FromStr;
use storage_db::STORAGE_INSTANCE_REF;
use tracing::*;

use crate::PeerNetwork;

pub struct P2PStorage {}

impl P2PStorage {
    fn get_peers_table(peer_table: &str) -> Option<Peers> {
        match STORAGE_INSTANCE_REF
            .key_value_db()
            .lock()
            .get(peer_table.as_bytes())
        {
            Ok(vlaue) => {
                if let Some(bytes) = vlaue {
                    match Message::parse_from_bytes(bytes.as_slice()) {
                        Ok(peers) => return Some(peers),
                        Err(err) => return None,
                    };
                }
            }
            Err(err) => return None,
        }

        None
    }

    fn put_tables(peer_table: &str, all_peers: &mut Peers) -> bool {
        match STORAGE_INSTANCE_REF.key_value_db().lock().put(
            peer_table.as_bytes().to_vec(),
            all_peers.write_to_bytes().unwrap(),
        ) {
            Ok(()) => return true,
            Err(err) => return false,
        }
    }

    pub fn create_peer(network: &PeerNetwork, peer_table: &str, address: &SocketAddr) -> bool {
        if !address.is_ipv4() || address.port() == 0 {
            error!(parent: network.span(),
                "failed to create peer.Invalid peer address {}",
                address.to_string()
            );
            return false;
        }
        let peer_count = P2PStorage::query_item(peer_table, address);
        if peer_count < 0 {
            error!(parent: network.span(),
                "failed to create peer. Unable to query the address {}",
                address.to_string()
            );
            return false;
        } else if peer_count > 0 {
            return true;
        }

        let mut values: Peer = Peer::new();
        values.set_num_failures(0);
        values.set_address(address.to_string());
        if !P2PStorage::update_item(peer_table, address, values) {
            error!(parent: network.span(), "ailed to insert a peer");
            return false;
        }
        true
    }

    pub fn query_top_item(peer_table: &str, limit: i64, records: &mut Peers) -> i64 {
        let mut peer_count = 0;
        if let Some(mut all) = P2PStorage::get_peers_table(peer_table) {
            for item in all.peers {
                if peer_count < limit {
                    records.peers.push(item);
                    peer_count += 1;
                }
            }
        }

        peer_count
    }

    fn query_item(peer_table: &str, address: &SocketAddr) -> i64 {
        let mut peer_count = 0;
        if let Some(mut all) = P2PStorage::get_peers_table(peer_table) {
            for item in all.peers {
                let mut ip_port = SocketAddr::from_str(item.get_address());
                if ip_port.is_err() {
                    continue;
                }
                if ip_port.unwrap() == *address {
                    peer_count += 1;
                }
            }
        }
        peer_count
    }

    pub fn clean_not_active_peers(network: &PeerNetwork, peer_table: &str) {
        if let Some(mut all) = P2PStorage::get_peers_table(peer_table) {
            let mut new_all: Peers = Peers::new();
            for item in all.get_peers().iter() {
                if item.get_num_failures() < 50 {
                    new_all.mut_peers().push(item.clone());
                }
            }

            if all.get_peers().len() > new_all.get_peers().len() {
                let ret = P2PStorage::put_tables(peer_table, &mut new_all);
                if !ret {
                    error!(parent: network.span(), "failed to write a new peer table");
                } else {
                    info!(parent: network.span(),
                        "cleaned {} inactive peers, left {} peers",
                        all.get_peers().len() - new_all.get_peers().len(),
                        new_all.get_peers().len()
                    );
                }
            }
        }
    }

    pub fn update_item(peer_table: &str, address: &SocketAddr, record: Peer) -> bool {
        if let Some(mut all) = P2PStorage::get_peers_table(peer_table) {
            let total_peers_count = all.get_peers().len();
            let mut peer_count = 0;

            for item in all.peers.iter_mut() {
                let result = SocketAddr::from_str(item.get_address());
                if result.is_err() {
                    continue;
                }
                let ip_port = result.unwrap();

                if ip_port == *address || ip_port.ip() == Ipv4Addr::UNSPECIFIED {
                    peer_count += 1;
                    if record.num_failures >= 0 {
                        item.set_num_failures(record.num_failures);
                    };
                    if record.next_attempt_time >= 0 {
                        item.set_next_attempt_time(record.next_attempt_time);
                    };
                    if record.active_time >= 0 {
                        item.set_active_time(record.active_time);
                    };
                    if record.connection_id >= 0 {
                        item.set_connection_id(record.connection_id);
                    };
                }
            }

            if peer_count == 0 && !(address.ip() == Ipv4Addr::UNSPECIFIED) {
                all.mut_peers().push(record);
            }

            let ret = P2PStorage::put_tables(peer_table, &mut all);
            if !ret {
                error!("failed to write the peer table");
            }
            return ret;
        }
        false
    }

    pub fn reset_peer_active(network: &PeerNetwork, peer_table: &str) {
        let mut all: Peers = Peers::default();
        let ret = P2PStorage::put_tables(peer_table, &mut all);
        if !ret {
            error!(parent: network.span(), "reset peer active failed to write the peer table");
        }
    }

    pub fn resolve_peer(network: &PeerNetwork, peer_table: &str, known_peers: &Vec<String>) {
        let mut resolved_ips: HashSet<SocketAddr> = HashSet::new();

        for iter in known_peers.clone() {
            let mut result = SocketAddr::from_str(iter.as_str());
            match result {
                Ok(addrss) => {
                    resolved_ips.insert(addrss);
                }
                Err(err) => {
                    let r = iter.to_socket_addrs();
                    match r {
                        Ok(v) => {
                            let arr: HashSet<_> = v.collect();
                            // info!("resolve peer {}--- {:?}", iter, arr);
                            resolved_ips.extend(arr);
                        }
                        Err(err) => {
                            error!("unable to resolve domain({})", iter.as_str());
                            continue;
                        }
                    }
                }
            }
        }
        for address in resolved_ips.iter() {
            // info!("all resolve peer --- {}", address);
            P2PStorage::create_peer(network, peer_table, address);
        }
    }
}
