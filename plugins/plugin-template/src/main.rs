use std::{io::stderr, sync::Arc};

use tlock_pdk::{
    dispatcher::Dispatcher,
    futures::executor::block_on,
    impl_rpc_handler,
    tlock_api::{RpcMethod, global},
    wasmi_pdk::{tracing::info, tracing_subscriber::fmt, transport::JsonRpcTransport},
};

struct MyPlugin {
    transport: Arc<JsonRpcTransport>,
}

impl MyPlugin {
    pub fn new(transport: Arc<JsonRpcTransport>) -> Self {
        Self {
            transport: transport,
        }
    }
}

impl_rpc_handler!(MyPlugin, global::Ping, |self, _params| {
    global::Ping.call(self.transport.clone(), ()).await?;
    Ok("pong".to_string())
});

fn main() {
    fmt().with_writer(stderr).init();
    info!("Starting plugin...");

    let reader = std::io::BufReader::new(::std::io::stdin());
    let writer = std::io::stdout();
    let transport = JsonRpcTransport::new(Box::new(reader), Box::new(writer));
    let transport = Arc::new(transport);

    let plugin = MyPlugin::new(transport.clone());
    let plugin = Arc::new(plugin);

    let mut dispatcher = Dispatcher::new(plugin);
    dispatcher.register::<global::Ping>();
    let dispatcher = Arc::new(dispatcher);

    block_on(async move {
        let _ = transport.process_next_line(Some(dispatcher)).await;
    });
}
