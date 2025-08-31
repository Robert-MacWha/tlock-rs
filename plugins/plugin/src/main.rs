use std::{
    io::{self, BufRead, Write},
    sync::Arc,
};

use tlock_pdk::{
    api::{Plugin, PluginApi, PluginNamespace, TlockNamespace},
    async_pipe::async_pipe,
    async_trait::async_trait,
    futures::{self, AsyncReadExt, io::BufReader},
    plugin_factory::PluginFactory,
    rpc_message::RpcErrorCode,
    runtime::spawn,
    transport::json_rpc_transport::JsonRpcTransport,
    typed_host::TypedHost,
    wasm_bindgen_futures::{future_to_promise, wasm_bindgen::JsValue},
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

        let count = find_primes(10);

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
    println!("Plugin started");
    let (stdin_reader, mut stdin_writer) = async_pipe();
    let (stdout_reader, stdout_writer) = async_pipe();

    let buf_reader = BufReader::new(stdin_reader);
    let transport = JsonRpcTransport::new(Box::new(buf_reader), Box::new(stdout_writer));
    let transport = Arc::new(transport);

    spawn(async move {
        let mut stdin = std::io::BufReader::new(io::stdin());
        let mut line = String::new();
        loop {
            line.clear();
            match stdin.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if stdin_writer.write(line.as_bytes()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    spawn(async move {
        let mut stdout = io::stdout();
        let mut buffer = [0u8; 1024];
        let mut buf_reader = BufReader::new(stdout_reader);
        loop {
            match buf_reader.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => {
                    if stdout.write(&buffer[..n]).is_err() {
                        break;
                    }
                    if stdout.flush().is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let host = TypedHost::new(transport.clone());
    let host = Arc::new(host);
    let plugin_instance = MyPlugin::new(host.clone());
    let plugin = Plugin(plugin_instance);
    let plugin = Arc::new(plugin);

    let runtime_future = async move {
        if let Err(e) = transport.process_next_line(Some(plugin)).await {
            println!("Transport error: {:?}", e);
        }
    };

    // Block until completion
    futures::executor::block_on(runtime_future);
}
