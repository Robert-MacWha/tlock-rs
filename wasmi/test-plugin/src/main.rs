use std::sync::Arc;

use wasmi_pdk::{
    api::RequestHandler, async_trait::async_trait, log, plugin_factory::PluginFactory,
    register_plugin, rpc_message::RpcErrorCode, serde_json::Value, transport::JsonRpcTransport,
};

struct MyPlugin {
    host: Arc<JsonRpcTransport>,
}

impl PluginFactory for MyPlugin {
    fn new(transport: Arc<JsonRpcTransport>) -> Self {
        Self { host: transport }
    }
}

#[async_trait]
impl RequestHandler<RpcErrorCode> for MyPlugin {
    async fn handle(&self, method: &str, params: Value) -> Result<Value, RpcErrorCode> {
        log::info!("Received method: {}, params: {:?}", method, params);

        match method {
            "ping" => {
                // Send a ping request, expect to receive a "pong" response.
                let id = 42;
                log::info!("Sending ping with id={}", id);
                let resp = self.host.call(id, "ping", Value::Null, None).await?;
                log::info!("Received response: {:?}", resp);

                if resp.id != id {
                    log::error!("Incorrect response id: expected {}, got {}", id, resp.id);
                    return Err(RpcErrorCode::InternalError);
                }

                if resp.result != Value::String("pong".to_string()) {
                    log::error!("Incorrect response result: {:?}", resp.result);
                    return Err(RpcErrorCode::InternalError);
                }

                Ok(Value::String("pong".to_string()))
            }
            _ => Err(RpcErrorCode::MethodNotFound),
        }
    }
}

register_plugin!(MyPlugin);
