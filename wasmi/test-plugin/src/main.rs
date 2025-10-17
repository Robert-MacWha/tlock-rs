use serde_json::{self, Value};
use std::{
    io::{stderr, stdin, stdout},
    sync::Arc,
};
use wasmi_pdk::{
    api::RequestHandler,
    async_trait::async_trait,
    futures::executor::block_on,
    rpc_message::RpcErrorCode,
    tracing::{error, info, level_filters::LevelFilter, trace},
    tracing_subscriber::fmt,
    transport::{JsonRpcTransport, Transport},
};

struct MyPlugin {
    host: Arc<JsonRpcTransport>,
}

impl MyPlugin {
    fn new(host: Arc<JsonRpcTransport>) -> Self {
        Self { host }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl RequestHandler<RpcErrorCode> for MyPlugin {
    async fn handle(&self, method: &str, params: Value) -> Result<Value, RpcErrorCode> {
        info!("Received method: {}, params: {:?}", method, params);

        match method {
            "ping" => {
                // Send a ping request, expect to receive a "pong" response.
                info!("Sending ping");
                let resp = self.host.call("ping", Value::Null).await?;
                info!("Received response: {:?}", resp);

                if resp.id != 0 {
                    error!("Incorrect response id: expected {}, got {}", 0, resp.id);
                    return Err(RpcErrorCode::InternalError);
                }

                if resp.result != Value::String("pong".to_string()) {
                    error!("Incorrect response result: {:?}", resp.result);
                    return Err(RpcErrorCode::InternalError);
                }

                info!("Ping successful, returning");
                Ok(Value::String("pong".to_string()))
            }
            "prime_sieve" => {
                let limit = params.as_u64().ok_or(RpcErrorCode::InvalidParams)? as usize;
                let primes = sieve_of_eratosthenes(limit);

                info!("Generated {} primes up to {}", primes.len(), limit);

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
                        error!("Incorrect response id: expected {}, got {}", i, resp.id);
                        return Err(RpcErrorCode::InternalError);
                    }

                    if resp.result != Value::Number(i.into()) {
                        error!("Incorrect response result: {:?}", resp.result);
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

fn main() {
    fmt()
        .with_max_level(LevelFilter::TRACE)
        .with_writer(stderr)
        .compact()
        .with_ansi(false)
        .without_time()
        .init();
    trace!("Starting plugin...");

    let reader = std::io::BufReader::new(stdin());
    let writer = stdout();
    let transport = JsonRpcTransport::new(Box::new(reader), Box::new(writer));
    let transport = Arc::new(transport);

    let plugin = MyPlugin::new(transport.clone());
    let plugin = Arc::new(plugin);

    let runtime_future = async move {
        let _ = transport.process_next_line(Some(plugin)).await;
    };

    block_on(runtime_future);
}
