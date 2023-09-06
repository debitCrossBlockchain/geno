use std::collections::HashSet;

use anyhow::{Ok, bail};
use network::{Endpoint, PeerNetwork};
use protos::common::ProtocolsMessage;





pub trait SyncNetworkInterace{
    fn select_peers(&self)->Option<HashSet<Endpoint>>;
    fn send_msg(&self, id:Endpoint, msg:ProtocolsMessage)->anyhow::Result<()>;
    fn broadcast_msg(&self, msg: ProtocolsMessage)->anyhow::Result<()>;
} 

pub struct SyncNetwork{
    inner: PeerNetwork
}

impl SyncNetwork{
    pub fn new(network: PeerNetwork)->Self{
        Self { inner: network }
    }

    pub fn peers(&self)->HashSet<Endpoint>{
        self.inner.conn_ids()
    }

    pub fn send_msg(&self, id:Endpoint, msg:ProtocolsMessage)->bool{
        self.inner.send_msg(id, msg)
    }   

    pub fn broadcast_msg(&self, msg: ProtocolsMessage)->bool{
        self.inner.broadcast_msg(msg)
    }
}

impl SyncNetworkInterace for SyncNetwork{
    fn select_peers(&self)->Option<HashSet<Endpoint>>{
        let ps = self.peers();
        if ps.is_empty(){
            None
        }else{
            Some(ps)
        }
    }
    fn send_msg(&self, id:Endpoint, msg:ProtocolsMessage)->anyhow::Result<()>{
        if self.send_msg(id, msg){
            Ok(())
        }else{
            bail!("send fail!")
        }
    }
    fn broadcast_msg(&self, msg: ProtocolsMessage)->anyhow::Result<()>{
        if self.broadcast_msg(msg){
            Ok(())
        }else{
            bail!("broadcast fail!")
        }
    }
}