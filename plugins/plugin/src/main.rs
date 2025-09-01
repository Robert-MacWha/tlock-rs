use std::{
    io::{self, BufReader},
    sync::Arc,
};

use tlock_pdk::{
    api::{Plugin, PluginApi, PluginNamespace, TlockNamespace},
    async_trait::async_trait,
    futures,
    plugin_factory::PluginFactory,
    rpc_message::RpcErrorCode,
    transport::json_rpc_transport::JsonRpcTransport,
    typed_host::TypedHost,
};
struct MyPlugin {
    host: Arc<TypedHost>,
}

impl PluginApi<RpcErrorCode> for MyPlugin {}

impl PluginFactory for MyPlugin {
    fn new(host: Arc<TypedHost>) -> Self {
        MyPlugin { host }
    }
}

#[async_trait]
impl TlockNamespace<RpcErrorCode> for MyPlugin {
    async fn ping(&self, message: String) -> Result<String, RpcErrorCode> {
        self.host.ping("Hello from plugin v2".to_string()).await?;

        let count = find_primes(10000);

        Ok(format!("Pong: message={}, primes={}", message, count))
    }
}

#[async_trait]
impl PluginNamespace<RpcErrorCode> for MyPlugin {
    async fn name(&self) -> Result<String, RpcErrorCode> {
        Ok("Test Async Plugin".to_string())
    }

    async fn version(&self) -> Result<String, RpcErrorCode> {
        Ok("1.0.0".to_string())
    }
}

fn find_primes(limit: usize) -> usize {
    let mut is_prime = vec![true; limit];
    let mut count = 0;

    for i in 2..limit {
        if is_prime[i] {
            count += 1;
            let mut j = i * i;
            while j < limit {
                is_prime[j] = false;
                j += i;
            }
        }
    }
    count
}

fn main() {
    let buf_reader = BufReader::new(io::stdin());
    let transport = JsonRpcTransport::new(Box::new(buf_reader), Box::new(io::stdout()));
    let transport = Arc::new(transport);

    let host = TypedHost::new(transport.clone());
    let host = Arc::new(host);
    let plugin_instance = MyPlugin::new(host.clone());
    let plugin = Plugin(plugin_instance);
    let plugin = Arc::new(plugin);

    let runtime_future = async move {
        let _ = transport.process_next_line(Some(plugin)).await;
    };

    futures::executor::block_on(runtime_future);

    println!("Plugin runtime finished");
}
