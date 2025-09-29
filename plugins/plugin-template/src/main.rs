use std::sync::Arc;

use tlock_pdk::{
    async_trait::async_trait,
    dispatcher::{Dispatcher, RpcHandler},
    futures::executor::block_on,
    tlock_api::{RpcMethod, global},
    wasmi_pdk::{rpc_message::RpcErrorCode, transport::JsonRpcTransport},
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

#[async_trait]
impl RpcHandler<global::Ping> for MyPlugin {
    async fn invoke(&self, _params: ()) -> Result<String, RpcErrorCode> {
        global::Ping.call(self.transport.clone(), ()).await?;

        Ok("pong".to_string())
    }
}

fn main() {
    stderrlog::new()
        .verbosity(stderrlog::LogLevelNum::Trace)
        .init()
        .unwrap();
    log::trace!("Starting plugin...");

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
