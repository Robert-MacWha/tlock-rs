use serde_json::{self, Value};
use std::{
    io::{stderr, stdin, stdout},
    sync::Arc,
};
use wasmi_pdk::{
    futures::executor::block_on,
    rpc_message::RpcError,
    server::ServerBuilder,
    tracing::{error, info, level_filters::LevelFilter, trace},
    tracing_subscriber::fmt,
    transport::{JsonRpcTransport, Transport},
};

async fn prime_sieve(_transport: Arc<JsonRpcTransport>, limit: u64) -> Result<Value, RpcError> {
    let limit = limit as usize;
    let primes = sieve_of_eratosthenes(limit);
    info!("Generated {} primes up to {}", primes.len(), limit);
    Ok(serde_json::json!({
        "count": primes.len(),
        "limit": limit
    }))
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

    let plugin = ServerBuilder::new(transport.clone());
    let plugin = plugin.with_method("ping", |_, _params: ()| async move {
        info!("Received ping request, sending pong response");
        Ok("pong".to_string())
    });
    let plugin = plugin.with_method("prime_sieve", prime_sieve);
    let plugin = plugin.with_method("many_echo", many_echo).finish();

    let plugin = Arc::new(plugin);

    let runtime_future = async move {
        let _ = transport.process_next_line(Some(plugin)).await;
    };

    block_on(runtime_future);
}
