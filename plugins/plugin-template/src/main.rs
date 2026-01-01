use std::io::stderr;

use tlock_pdk::{
    runner::PluginRunner,
    tlock_api::{RpcMethod, global},
    wasmi_plugin_pdk::{rpc_message::RpcError, transport::Transport},
};
use tracing::info;
use tracing_subscriber::fmt;

async fn ping(transport: Transport, _: ()) -> Result<String, RpcError> {
    global::Ping.call_async(transport, ()).await?;
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

    PluginRunner::new().with_method(global::Ping, ping).run();
}
