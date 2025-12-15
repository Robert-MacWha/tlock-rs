use std::{io::stderr, sync::Arc};

use tlock_pdk::{
    server::PluginServer,
    tlock_api::{RpcMethod, global},
    wasmi_plugin_pdk::{rpc_message::RpcError, transport::JsonRpcTransport},
};
use tracing::info;
use tracing_subscriber::fmt;

async fn ping(transport: Arc<JsonRpcTransport>, _: ()) -> Result<String, RpcError> {
    global::Ping.call(transport, ()).await?;
    Ok("pong".to_string())
}

fn main() {
    fmt()
        .with_writer(stderr)
        .without_time()
        .with_ansi(false)
        .compact()
        .init();
    info!("Starting plugin...");

    PluginServer::new_with_transport()
        .with_method(global::Ping, ping)
        .run();
}
