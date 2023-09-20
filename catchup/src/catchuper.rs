

use crate::network::CatchupNetworkInterace;
use crate::notification::{TimerNotificationReceiver, BroadcastSender};
use crate::storage_executor::StorageExecutorInterface;
use crate::catchup_status::{CatchupStatus, Peers};

use protos::{
    common::{
    ProtocolsActionMessageType, ProtocolsMessage, ProtocolsMessageType,},
    ledger::{SyncBlockRequest, SyncBlockResponse, SyncChain, SyncChainStatus, TransactionSignBrodcast}
};
use utils::{
    parse::ProtocolParser,
    timer_manager::{TimterEventParam, TimerEventType, TimerManager},
    general::self_chain_id,
};
use network::{Endpoint, LocalBusSubscriber, ReturnableProtocolsMessage};
use types::SignedTransaction;

use crossbeam_channel::{RecvError, select, bounded};
use protobuf::{RepeatedField, Message}; 
use tracing::error;


pub struct Catchuper<S, N>{
    status: CatchupStatus,
    peers: Peers,
    executor: S,
    network: N,
    timer_notify: TimerNotificationReceiver, 
    txns_nofity: BroadcastSender,
}

impl<S, N> Catchuper <S, N> 
    where
        S:StorageExecutorInterface+Send + 'static,
        N:CatchupNetworkInterace+Send + 'static
    {
    pub fn create_and_start(
        network: N, 
        executor: S, 
        txns_nofity: BroadcastSender,
    ){

        let (sender, timer_notify) = bounded(1024);
        let _id: i64 = TimerManager::instance().new_repeating_timer(
            chrono::Duration::seconds(5),
            sender,
            TimerEventType::LedgerSync,
            None,
        );
        let mut catchup = Catchuper::new(network, executor, timer_notify, txns_nofity);
        
        //get last blockheader
        let subscriber = catchup.network.add_subscribers(
            &[ProtocolsMessageType::SYNCCHAIN, 
              ProtocolsMessageType::SYNCBLOCK,
              ProtocolsMessageType::TRANSACTION,
            ]);

        let _ = std::thread::spawn(move || loop {
            catchup.start(subscriber.clone());
        });

    }

    pub fn new(
        network: N, 
        executor: S, 
        timer_notify: TimerNotificationReceiver,
        txns_nofity: BroadcastSender,) -> Self{
        
        Self{
            status:CatchupStatus::default(),
            peers: Peers::default(),
            executor,
            network,
            timer_notify,
            txns_nofity,
        }
    }
 
    pub fn start(&mut self, subscriber:LocalBusSubscriber<ProtocolsMessageType, ReturnableProtocolsMessage>){
        select! {
            recv(subscriber.inbox) -> msg =>{
                self.handle_catchup_notification(msg)
            }
            recv(self.timer_notify) -> msg =>{
                self.handle_timer(msg)
            }
        }
    }

    fn handle_catchup_notification(&mut self, msg: Result<(ProtocolsMessageType, (Endpoint, ProtocolsMessage)), RecvError>){
        match msg {
            Ok((proto_type, (peer_endpoint,proto_message)))=>{
                match proto_type {
                    ProtocolsMessageType::SYNCCHAIN => {
                        match proto_message.get_action() {
                            ProtocolsActionMessageType::BROADCAST => {
                                self.handle_catchup_chain_broadcast(peer_endpoint, &proto_message);
                            },
                            ProtocolsActionMessageType::RESPONSE => {
                                self.handle_catchup_chain_response(peer_endpoint, &proto_message);
                            },
                            _=>(),
                        }
                    }

                    ProtocolsMessageType::SYNCBLOCK => {
                        match proto_message.get_action() {
                            ProtocolsActionMessageType::REQUEST => {
                                self.handle_catchup_block_reqest(peer_endpoint, &proto_message);
                            },
                            ProtocolsActionMessageType::RESPONSE => {
                                self.handle_catchup_block_response(peer_endpoint, &proto_message);
                            },
                            _=>(),
                        }
                    }

                    ProtocolsMessageType::TRANSACTION => {
                        match proto_message.get_action() {
                            ProtocolsActionMessageType::BROADCAST => {
                                self.handle_txns_broadcast(peer_endpoint, &proto_message);
                            },
                            _=>(),
                        }
                    }

                    _ => {}
                }
            }
            Err(e)=>{
                error!("catchup receive error: {}",e);
            }
        }
    }

    fn handle_timer(&mut self, msg: Result<TimterEventParam, RecvError>){
        match msg {
            Ok(param)=>{
                match param.event_type{
                    TimerEventType::LedgerSync =>{
                        self.catchup_chain();
                        if !self.status.is_catchuping(){
                            self.catchup_block(None);
                        }
                    }
                    _=>{}
                }
            }
            Err(e)=>{
                error!("catchup timer error: {}",e);
            }
        }
    }

    pub fn catchup_block(&mut self, peer_id: Option<Endpoint>,) {

        let active_peer = if peer_id.is_none(){
            let active_peer = self.peers.select_peer();
            if active_peer.is_none() {
                return;
            }
            *active_peer.unwrap().0
        }else{
            peer_id.unwrap()
        };
        
        let cur_height = match self.executor.get_block_height(){
           Ok(Some(h))  => h,
           Ok(None)  => return,
           Err(e) => return,
        };
    
        let mut req: SyncBlockRequest = SyncBlockRequest::new();
        req.set_begin(cur_height as i64);
        req.set_end(0);
        req.set_requestid(0);

        let mut message = protos::common::ProtocolsMessage::new();
        message.set_msg_type(protos::common::ProtocolsMessageType::SYNCBLOCK);
        message.set_action(protos::common::ProtocolsActionMessageType::REQUEST);
        message.set_data(req.write_to_bytes().unwrap());

        let _ = self.network.send_msg(active_peer, message);
    }

    fn handle_catchup_block_reqest(
        &self,
        peer_id: Endpoint,
        protocol_msg: &ProtocolsMessage,
    ) {
        let block_req :SyncBlockRequest = match ProtocolParser::deserialize(protocol_msg.get_data()) {
            Ok(value) => value,
            Err(e) => return,
        };
    
        if block_req.get_chain_id() != self_chain_id() {
            return;
        }

        let last_h = match self.executor.get_block_height(){
            Ok(Some(v)) => v,
            Ok(None) => return,
            Err(e) => return,
        };
    
        let begin = block_req.get_begin() as u64;
        //let end = block_req.get_end() as u64;

        let end_rep = if last_h >= begin + 5{
            begin + 5
        }else if last_h < begin{
            return;
        }else{
            last_h
        };

        let mut block_rep = SyncBlockResponse::new();
        let mut blocks = vec![];
        for h in begin..=end_rep{
            match self.executor.get_block(h){
                Ok(Some(v)) => blocks.push(v),
                Ok(None) => {
                    block_rep.set_finish(true);
                    break
                },
                Err(e) => return,
            };
        }
        let block_len = blocks.len();
        
    
        if block_len != 0 {

            //send Ledger
            block_rep.set_chain_id(self_chain_id());
            block_rep.set_number(block_len as i64);
            block_rep.set_blocks(RepeatedField::from(blocks));

            let mut message = protos::common::ProtocolsMessage::new();
            message.set_msg_type(protos::common::ProtocolsMessageType::SYNCBLOCK);
            message.set_action(protos::common::ProtocolsActionMessageType::RESPONSE);
            message.set_data(block_rep.write_to_bytes().unwrap());
            
            let _ =  self.network.send_msg(peer_id, message);
            
        }
    }
    
    fn handle_catchup_block_response(&mut self, peer_id: Endpoint, protocol_msg: &ProtocolsMessage) {

        
        let block_rep :SyncBlockResponse = match ProtocolParser::deserialize(protocol_msg.get_data()) {
            Ok(value) => value,
            Err(e) => return,
        };
    
        self.status.catchup_ing(0);

        if block_rep.get_chain_id() != self_chain_id() {
            self.status.catchup_prepare();
            return;
        }

        let last_h = match self.executor.get_block_height(){
            Ok(Some(v)) => v,
            Ok(None) => {
                self.status.catchup_prepare();
                return},
            Err(e) => {
                self.status.catchup_prepare();
                return},
        };
    
        let blocks = block_rep.get_blocks();
 
        if blocks[0].get_header().get_height() == last_h + 1{
            blocks.iter().map(|block|{
                self.execute_verify_block(block.to_owned());
            });
        }else{
            if blocks[0].get_header().get_height() > last_h + 1{
                //cache blocks
                ()
            }else{
                if blocks[blocks.len() -1].get_header().get_height() >= last_h + 1{
                    let (n, _):(Vec<_>, Vec<_>) = 
                    blocks.iter().partition(|block|block.get_header().get_height() >= last_h + 1);
                    n.iter().map(|block|{
                        self.execute_verify_block(block.to_owned().to_owned());
                    });
                }
            }
        }

        if !block_rep.get_finish() {
            self.catchup_block(Some(peer_id));
        }else{
            self.status.catchup_done();
        }

    }
    
    pub fn catchup_chain(&mut self) {
        let active_peers = self.network.select_peers();
        if active_peers.is_none() {
            return;
        }
        
        let cur_height = match self.executor.get_block_height(){
           Ok(Some(h))  => h,
           Ok(None)  => return,
           Err(e) => return,
        };
    
        let mut chain: SyncChain = SyncChain::new();
        chain.set_height(cur_height as i64);
        chain.set_hash(0);
        chain.set_chain_id(self_chain_id());

        let mut message = protos::common::ProtocolsMessage::new();
        message.set_msg_type(protos::common::ProtocolsMessageType::SYNCCHAIN);
        message.set_action(protos::common::ProtocolsActionMessageType::BROADCAST);
        message.set_data(chain.write_to_bytes().unwrap());

        let _ = self.network.broadcast_msg(message);
    }

    fn handle_catchup_chain_broadcast(
        &mut self,
        peer_id: Endpoint,
        protocol_msg: &ProtocolsMessage,
    ) {
        let chainrequest :SyncChain = match ProtocolParser::deserialize(protocol_msg.get_data()) {
            Ok(value) => value,
            Err(e) => return,
        };

        let ch = chainrequest.get_height() as u64;

        let cur_height = match self.executor.get_block_height(){
            Ok(Some(v)) => v,
            Ok(None) => return,
            Err(_) => return,
        };

        if cur_height < ch{
            self.peers.insert_peer(peer_id, ch as u64);
            return
        }

        let mut chainstatus: SyncChainStatus = SyncChainStatus::new();
        chainstatus.set_height(cur_height as i64);
        chainstatus.set_hash("0".to_string());
        chainstatus.set_chain_id(self_chain_id());

        let mut message = protos::common::ProtocolsMessage::new();
        message.set_msg_type(protos::common::ProtocolsMessageType::SYNCCHAIN);
        message.set_action(protos::common::ProtocolsActionMessageType::RESPONSE);
        message.set_data(chainstatus.write_to_bytes().unwrap());

        let _ = self.network.send_msg(peer_id,message);

    }

    fn handle_catchup_chain_response(
        &mut self,
        peer_id: Endpoint,
        protocol_msg: &ProtocolsMessage,
    ) {
        let chainstatus :SyncChainStatus = match ProtocolParser::deserialize(protocol_msg.get_data()) {
            Ok(value) => value,
            Err(e) => {
                self.peers.update_score_error(peer_id);
                return},
        };
        let h = chainstatus.get_height();

        self.peers.insert_peer(peer_id, h as u64);
        self.peers.update_score_success(peer_id);
        self.peers.remmove_ignored();
    }

    fn execute_verify_block(
        &self,
        block: protos::ledger::Ledger,
    )->Result<(),()>{
        self.executor.execute_verify_block(block);
        Ok(())
    }

    fn handle_txns_broadcast(
        &mut self,
        peer_id: Endpoint,
        protocol_msg: &ProtocolsMessage,
    ) {
        let mut txns :TransactionSignBrodcast = match ProtocolParser::deserialize(protocol_msg.get_data()) {
            Ok(value) => value,
            Err(e) => return,
        };

        let txns = txns
            .get_transactions()
            .iter()
            .filter_map(|x| {
                SignedTransaction::try_from(x.to_owned()).ok()
            })
            .collect::<Vec<_>>();

        if let Err(e) = self.txns_nofity.unbounded_send(txns) {
            error!("broadcast transaction send to tx-pool error({:?})", e);
        }

    }
}


