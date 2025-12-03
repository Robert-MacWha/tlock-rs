use rand::Rng;
use serde_json::{self, Value};
use std::{io::stderr, sync::Arc};
use wasmi_pdk::{
    rpc_message::RpcError,
    server::PluginServer,
    tracing::{error, info, level_filters::LevelFilter, trace},
    tracing_subscriber::fmt,
    transport::{JsonRpcTransport, Transport},
};

async fn get_random_number(_: Arc<JsonRpcTransport>, _: ()) -> Result<Value, RpcError> {
    let mut rng = rand::rng();
    let random_number: u64 = rng.random();
    Ok(Value::Number(random_number.into()))
}

async fn get_time(_: Arc<JsonRpcTransport>, _: ()) -> Result<Value, RpcError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| {
            error!("System time before UNIX EPOCH: {}", e);
            RpcError::InternalError
        })?;
    Ok(Value::Number(now.as_secs().into()))
}

async fn sleep(_: Arc<JsonRpcTransport>, duration_ms: u64) -> Result<(), RpcError> {
    info!("Sleeping for {} milliseconds", duration_ms);
    tokio::time::sleep(std::time::Duration::from_millis(duration_ms)).await;
    info!("Woke up after sleeping for {} milliseconds", duration_ms);
    Ok(())
}

async fn many_echo(transport: Arc<JsonRpcTransport>, limit: u64) -> Result<(), RpcError> {
    for i in 0..limit {
        let resp = transport.call("echo", Value::Number(i.into())).await?;

        if resp.id != i {
            error!("Incorrect response id: expected {}, got {}", i, resp.id);
            return Err(RpcError::InternalError);
        }

        if resp.result != Value::Number(i.into()) {
            error!("Incorrect response result: {:?}", resp.result);
            return Err(RpcError::InternalError);
        }
    }

    Ok(())
}

async fn prime_sieve(_transport: Arc<JsonRpcTransport>, limit: u64) -> Result<Value, RpcError> {
    let limit = limit as usize;
    let primes = sieve_of_eratosthenes(limit);
    info!("Generated {} primes up to {}", primes.len(), limit);
    Ok(serde_json::json!({
        "count": primes.len(),
        "limit": limit
    }))
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

    PluginServer::new_with_transport()
        .with_method("ping", |_, _params: ()| async move {
            info!("Received ping request, sending pong response");
            Ok("pong".to_string())
        })
        .with_method("get_random_number", get_random_number)
        .with_method("get_time", get_time)
        .with_method("sleep", sleep)
        .with_method("many_echo", many_echo)
        .with_method("prime_sieve", prime_sieve)
        .run();
}
