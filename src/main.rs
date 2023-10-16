use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use catchup::{catchuper::Catchuper, network::CatchupNetwork, storage_executor::StoreageExecutor};
use configure::CONFIGURE_INSTANCE_REF;
use consensus_pbft::bootstrap::start_consensus;
use executor::BlockExecutor;
use jsonrpc::bootstrap::start_jsonrpc_service;
use network::{NetworkConfigType, PeerNetwork};
use syscontract::{
    contract_factory::initialize_system_contract_factory,
    system_address::initialize_syscontract_address,
};
use tx_pool::start_txpool_service;
use utils::{logger::LogUtil, timer_manager::initialize_timer_manager};

fn main() {
    let _guard = LogUtil::init("./log", "app.log", "setting/log_filter.txt").unwrap();

    if let Err(err) = geno_cmd::cli::run() {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
    initialize_syscontract_address();
    initialize_system_contract_factory();
    initialize_timer_manager();

    if let Err(e) = BlockExecutor::block_initialize() {
        eprintln!("start block error:{}", e);
        std::process::exit(1);
    }

    let network = PeerNetwork::start_service("peers", NetworkConfigType::Normal);
    let network_consensus =
        PeerNetwork::start_service("consensus_peers", NetworkConfigType::Consensus);

    let (jsonrpc_runtime, ws_runtime, jsonrpc_to_txpool_receiver, ws_event_sender) =
        start_jsonrpc_service(&CONFIGURE_INSTANCE_REF.json_rpc);

    let (txpool_runtime, broadcast_tx_sender, consensus_committed_sender) = start_txpool_service(
        &CONFIGURE_INSTANCE_REF.tx_pool,
        jsonrpc_to_txpool_receiver,
        network.clone(),
    );

    Catchuper::create_and_start(
        CatchupNetwork::new(network.clone()),
        StoreageExecutor::new(BlockExecutor {}),
        broadcast_tx_sender,
    );

    start_consensus(
        network,
        network_consensus,
        consensus_committed_sender,
        ws_event_sender,
    );

    let term = Arc::new(AtomicBool::new(false));
    while !term.load(Ordering::Acquire) {
        std::thread::park();
    }
}
