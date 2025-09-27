use std::sync::Arc;

use tlock_pdk::{
    async_trait::async_trait,
    futures::executor::block_on,
    stderrlog,
    wasmi_pdk::{
        api::RequestHandler, rpc_message::RpcErrorCode, serde_json::Value,
        transport::JsonRpcTransport,
    },
};

struct MyPlugin {
    host: Arc<JsonRpcTransport>,
}

impl MyPlugin {
    pub fn new(host: Arc<JsonRpcTransport>) -> Self {
        Self { host }
    }
}

#[async_trait]
impl RequestHandler<RpcErrorCode> for MyPlugin {
    async fn handle(&self, method: &str, params: Value) -> Result<Value, RpcErrorCode> {
        log::info!("Received request: method={}, params={}", method, params);

        match method {
            "ping" => Ok(Value::String("pong".to_string())),
            _ => Err(RpcErrorCode::MethodNotFound),
        }
    }
}

fn main() {
    stderrlog::new()
        .verbosity(::tlock_pdk::wasmi_pdk::stderrlog::LogLevelNum::Trace)
        .init()
        .unwrap();
    log::info!("Starting plugin...");

    let reader = std::io::BufReader::new(::std::io::stdin());
    let writer = std::io::stdout();
    let transport = JsonRpcTransport::new(Box::new(reader), Box::new(writer));
    let transport = Arc::new(transport);

    let plugin = Arc::new(MyPlugin::new(transport.clone()));

    block_on(async move {
        let _ = transport.process_next_line(Some(plugin)).await;
    });
}
