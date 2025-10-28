use std::{io::stderr, sync::Arc};

use tlock_pdk::{
    futures::executor::block_on,
    server::ServerBuilder,
    tlock_api::{RpcMethod, global},
    wasmi_pdk::{
        rpc_message::RpcError, tracing::info, tracing_subscriber::fmt,
        transport::JsonRpcTransport,
    },
};

async fn ping(transport: Arc<JsonRpcTransport>, _: ()) -> Result<String, RpcError> {
    global::Ping.call(transport, ()).await?;
    Ok("pong".to_string())
}

fn main() {
    fmt().with_writer(stderr).init();
    info!("Starting plugin...");

    let reader = std::io::BufReader::new(::std::io::stdin());
    let writer = std::io::stdout();
    let transport = JsonRpcTransport::new(Box::new(reader), Box::new(writer));
    let transport = Arc::new(transport);

    let plugin = ServerBuilder::new(transport.clone())
        .with_method(global::Ping, ping)
        .finish();

    let plugin = Arc::new(plugin);

    block_on(async move {
        let _ = transport.process_next_line(Some(plugin)).await;
    });
}
