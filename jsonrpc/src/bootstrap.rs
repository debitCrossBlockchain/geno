use crate::{
    errors::JsonRpcError,
    methods::{build_registry, RpcRegistry},
    request::JsonRpcRequest,
    response::{JsonRpcResponse, HEADER_CHAIN_ID},
    service::JsonRpcService,
    ws_connections::{process_publish_event, PublishEvent, WsConnections},
    ws_handler::ws_handler,
};
use anyhow::Result;
use futures::future::{join_all, Either};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    runtime::{Builder, Runtime},
    sync::mpsc,
};
use warp::{http::header, reject::Reject, Filter, Reply};

pub fn start_jsonrpc_service(
    config: &configure::JsonRpcConfig,
) -> (Runtime, mpsc::UnboundedSender<PublishEvent>) {
    let runtime = Builder::new_multi_thread()
        .thread_name("json-rpc")
        .enable_all()
        .build()
        .expect("[json-rpc] failed to create runtime");

    let collections = WsConnections::new(utils::general::self_chain_id());
    let collections_clone = collections.clone();
    let (event_sender, event_receiver) = mpsc::unbounded_channel::<PublishEvent>();
    let registry = Arc::new(build_registry());
    let service = JsonRpcService::new(config);

    let base_route = warp::any()
        .and(warp::post())
        .and(warp::header::exact("content-type", "application/json"))
        .and(warp::body::content_length_limit(
            config.content_length_limit as u64,
        ))
        .and(warp::body::json())
        .and(warp::any().map(move || service.clone()))
        .and(warp::any().map(move || Arc::clone(&registry)))
        .and_then(http_handler)
        .with(
            warp::cors()
                .allow_any_origin()
                .allow_methods(vec!["POST"])
                .allow_headers(vec![header::CONTENT_TYPE]),
        );

    let route_v1 = warp::path::path("v1")
        .and(warp::path::end())
        .and(base_route);

    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(warp::addr::remote())
        .and(warp::path::param())
        .and(warp::any().map(move || collections.clone()))
        .map(
            |ws: warp::ws::Ws,
             remote: Option<SocketAddr>,
             api_key: String,
             collections: WsConnections| {
                ws.on_upgrade(move |socket| ws_handler(socket, remote, api_key, collections))
            },
        );

    let health_route = warp::path!("-" / "healthy")
        .and(warp::path::end())
        .and_then(health_handler);

    let routes = health_route.or(route_v1.or(ws_route));

    let _guard = runtime.enter();
    let server = match &config.tls_cert_path {
        None => Either::Left(warp::serve(routes).bind(config.address.clone())),
        Some(cert_path) => Either::Right(
            warp::serve(routes)
                .tls()
                .cert_path(cert_path)
                .key_path(config.tls_key_path.as_ref().unwrap())
                .bind(config.address.clone()),
        ),
    };

    runtime
        .handle()
        .spawn(process_publish_event(collections_clone, event_receiver));
    runtime.handle().spawn(server);
    (runtime, event_sender)
}

async fn http_handler(
    data: serde_json::Value,
    service: JsonRpcService,
    registry: Arc<RpcRegistry>,
) -> Result<warp::reply::Response, warp::Rejection> {
    let chain_id = service.chain_id.clone();
    let resp = Ok(if let serde_json::Value::Array(requests) = data {
        match service.check_batch_size_limit(requests.len()) {
            Ok(_) => {
                let futures = requests
                    .into_iter()
                    .map(|req| request_handler(req, service.clone(), Arc::clone(&registry)));
                let responses = join_all(futures).await;
                warp::reply::json(&responses)
            }
            Err(err) => {
                let mut resp = JsonRpcResponse::new(chain_id.clone());
                resp.error = Some(err);
                warp::reply::json(&resp)
            }
        }
    } else {
        let resp = request_handler(data, service, registry).await;
        warp::reply::json(&resp)
    });

    let mut http_response = resp.into_response();
    let headers = http_response.headers_mut();
    headers.insert(
        HEADER_CHAIN_ID,
        header::HeaderValue::from_str(&chain_id).unwrap(),
    );
    Ok(http_response)
}

async fn request_handler(
    value: serde_json::Value,
    service: JsonRpcService,
    registry: Arc<RpcRegistry>,
) -> JsonRpcResponse {
    let mut response = JsonRpcResponse::new(service.chain_id.clone());

    let request: JsonRpcRequest = match serde_json::from_value(value) {
        Ok(req) => req,
        Err(_) => {
            response.error = Some(JsonRpcError::invalid_jsonrpc_format());
            return response;
        }
    };
    if let Err(err) = check_jsonrpc_format(&request) {
        response.error = Some(err);
        return response;
    }
    response.id = Some(serde_json::Value::Number(request.id.into()));

    match registry.get(&request.method) {
        Some(handler) => match handler(service, request).await {
            Ok(result) => {
                response.result = Some(result);
                response.error = Some(JsonRpcError::no_error());
            }
            Err(err) => {
                response.error = Some(
                    err.downcast_ref::<JsonRpcError>()
                        .cloned()
                        .unwrap_or_else(|| JsonRpcError::internal_error(err.to_string())),
                )
            }
        },
        None => response.error = Some(JsonRpcError::method_not_found()),
    }

    response
}

fn check_jsonrpc_format(request: &JsonRpcRequest) -> Result<(), JsonRpcError> {
    if request.jsonrpc != "2.0" || request.method.trim().is_empty() {
        return Err(JsonRpcError::invalid_jsonrpc_format());
    }
    Ok(())
}

async fn health_handler() -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    Ok(Box::new("ok"))
}
