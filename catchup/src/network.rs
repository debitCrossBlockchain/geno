use std::collections::HashSet;

use anyhow::{Ok, bail};
use network::{Endpoint, PeerNetwork, LocalBusSubscriber, ReturnableProtocolsMessage};
use protos::common::{ProtocolsMessage, ProtocolsMessageType};





pub trait CatchupNetworkInterace{
    fn add_subscribers(&self, topics: &[ProtocolsMessageType])->LocalBusSubscriber<ProtocolsMessageType, ReturnableProtocolsMessage>;
    fn select_peers(&self)->Option<HashSet<Endpoint>>;
    fn send_msg(&self, id:Endpoint, msg:ProtocolsMessage)->anyhow::Result<()>;
    fn broadcast_msg(&self, msg: ProtocolsMessage)->anyhow::Result<()>;
} 

pub struct CatchupNetwork{
    inner: PeerNetwork
}

impl CatchupNetwork{
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

    fn add_subscribers(&self, topics: &[ProtocolsMessageType])->LocalBusSubscriber<ProtocolsMessageType, ReturnableProtocolsMessage>{
        self.inner.add_subscribers(topics)
    } 
}

impl CatchupNetworkInterace for CatchupNetwork{
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
    fn add_subscribers(&self, topics: &[ProtocolsMessageType])->LocalBusSubscriber<ProtocolsMessageType, ReturnableProtocolsMessage>{
        self.add_subscribers(topics)
    }
}