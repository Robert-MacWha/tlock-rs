use std::sync::Arc;

use wasmi_pdk::{
    api::RequestHandler,
    async_trait::async_trait,
    log,
    plugin_factory::PluginFactory,
    register_plugin,
    rpc_message::RpcErrorCode,
    serde_json::{self, Value},
    transport::{JsonRpcTransport, Transport},
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
                log::info!("Sending ping");
                let resp = self.host.call("ping", Value::Null).await?;
                log::info!("Received response: {:?}", resp);

                if resp.id != 0 {
                    log::error!("Incorrect response id: expected {}, got {}", 0, resp.id);
                    return Err(RpcErrorCode::InternalError);
                }

                if resp.result != Value::String("pong".to_string()) {
                    log::error!("Incorrect response result: {:?}", resp.result);
                    return Err(RpcErrorCode::InternalError);
                }

                log::info!("Ping successful, returning");
                Ok(Value::String("pong".to_string()))
            }
            "prime_sieve" => {
                let limit = params.as_u64().ok_or(RpcErrorCode::InvalidParams)? as usize;
                let primes = sieve_of_eratosthenes(limit);

                log::info!("Generated {} primes up to {}", primes.len(), limit);

                Ok(serde_json::json!({
                    "count": primes.len(),
                    "limit": limit
                }))
            }
            "many_echo" => {
                let limit = params.as_u64().ok_or(RpcErrorCode::InvalidParams)? as usize;
                for i in 0..limit {
                    let resp = self.host.call("echo", Value::Number(i.into())).await?;

                    if resp.id != i as u64 {
                        log::error!("Incorrect response id: expected {}, got {}", i, resp.id);
                        return Err(RpcErrorCode::InternalError);
                    }

                    if resp.result != Value::Number(i.into()) {
                        log::error!("Incorrect response result: {:?}", resp.result);
                        return Err(RpcErrorCode::InternalError);
                    }
                }

                Ok(Value::Null)
            }
            _ => Err(RpcErrorCode::MethodNotFound),
        }
    }
}

fn sieve_of_eratosthenes(limit: usize) -> Vec<usize> {
    if limit < 2 {
        return vec![];
    }

    let mut is_prime = vec![true; limit + 1];
    is_prime[0] = false;
    is_prime[1] = false;

    for i in 2..=((limit as f64).sqrt() as usize) {
        if is_prime[i] {
            for j in ((i * i)..=limit).step_by(i) {
                is_prime[j] = false;
            }
        }
    }

    (2..=limit).filter(|&i| is_prime[i]).collect()
}

register_plugin!(MyPlugin);
