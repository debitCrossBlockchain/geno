use crypto::{digest::Digest, sha3::Sha3};
use local_ip_address::local_ip;
use message_io::network::Endpoint;
use protobuf::Message;
use protos::common::ProtocolsMessage;
use std::collections::HashSet;
use std::net::{SocketAddr, ToSocketAddrs};
use std::str::FromStr;
use std::time::Instant;
use std::{
    convert::TryInto,
    io::{self, Read},
    net::UdpSocket,
};

pub const P2P_LIMIT_SIZE: u32 = 20 * 1024 * 1024;
pub const PEER_DB_COUNT: usize = 5000;

pub struct P2PUtils;

impl P2PUtils {
    pub fn get_hash(value: &[u8]) -> String {
        let mut hasher = Sha3::sha3_256();
        hasher.input(value);
        hasher.result_str()
    }

    pub fn generate_hash(name: &str, ntype: i32, seq: u64) -> String {
        format!(
            "{}-{}-{}-{}",
            name,
            ntype,
            chrono::Local::now().timestamp_micros(),
            seq
        )
    }

    pub fn msg_has_hash(msg: &ProtocolsMessage) -> bool {
        msg.get_hash().len() > 0
    }

    pub fn get_local_addr() -> Option<String> {
        let socket = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(_) => return None,
        };

        match socket.connect("8.8.8.8:80") {
            Ok(()) => (),
            Err(_) => return None,
        };

        return match socket.local_addr() {
            Ok(addr) => Some(addr.ip().to_string()),
            Err(_) => None,
        };
    }

    pub fn resolve_address(addr: String) -> HashSet<SocketAddr> {
        let mut resolved_ips: HashSet<SocketAddr> = HashSet::new();

        let mut result = SocketAddr::from_str(addr.as_str());
        match result {
            Ok(addrss) => {
                resolved_ips.insert(addrss);
            }
            Err(err) => {
                let r = addr.to_socket_addrs();
                match r {
                    Ok(v) => {
                        let arr: HashSet<_> = v.collect();
                        resolved_ips.extend(arr);
                    }
                    Err(err) => {}
                }
            }
        }
        resolved_ips
    }

    pub fn get_local_address() -> Option<String> {
        match local_ip() {
            Ok(my_local_ip) => Some(my_local_ip.to_string()),
            Err(e) => None,
        }
    }

    pub fn read_len_prefixed_message<R: io::Read, const N: usize>(
        reader: &mut R,
    ) -> io::Result<Option<Vec<u8>>> {
        let mut len_arr = [0u8; N];
        if reader.read_exact(&mut len_arr).is_err() {
            return Ok(None);
        }
        let payload_len = match N {
            2 => u16::from_le_bytes(len_arr[..].try_into().unwrap()) as usize,
            4 => u32::from_le_bytes(len_arr[..].try_into().unwrap()) as usize,
            _ => unreachable!(),
        };

        if payload_len == 0 {
            return Err(io::ErrorKind::InvalidData.into());
        }

        let mut buffer = vec![0u8; payload_len];
        if reader
            .take(payload_len as u64)
            .read_exact(&mut buffer)
            .is_err()
        {
            Ok(None)
        } else {
            Ok(Some(buffer))
        }
    }

    pub fn prefix_with_len(len_size: usize, message: &[u8]) -> Vec<u8> {
        let mut vec = Vec::with_capacity(len_size + message.len());

        match len_size {
            2 => vec.extend_from_slice(&(message.len() as u16).to_le_bytes()),
            4 => vec.extend_from_slice(&(message.len() as u32).to_le_bytes()),
            _ => unreachable!(),
        }

        vec.extend_from_slice(message);
        vec
    }

    pub fn read_message<R: io::Read>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
        let vec = Self::read_len_prefixed_message::<R, 4>(reader)?;
        Ok(vec)
    }

    pub fn proto_msg_deserialize(bytes: &[u8]) -> anyhow::Result<protos::common::ProtocolsMessage> {
        let result: protobuf::ProtobufResult<protos::common::ProtocolsMessage> =
            protobuf::Message::parse_from_bytes(bytes);
        match result {
            Ok(msg) => Ok(msg),
            Err(err) => Err(anyhow::anyhow!("proto deserialize error {}", err)),
        }
    }

    pub fn proto_msg_serialize(msg: protos::common::ProtocolsMessage) -> Vec<u8> {
        msg.write_to_bytes().unwrap()
    }
}
