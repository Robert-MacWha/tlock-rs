use tlock_pdk::{
    api::{PluginApi, PluginNamespace, TlockNamespace},
    plugin_factory::PluginFactory,
    register_plugin,
    rpc_message::RpcErrorCode,
    typed_host::TypedHost,
};
struct MyPlugin<'a> {
    host: &'a TypedHost<'a>,
}

impl PluginApi<RpcErrorCode> for MyPlugin<'_> {}

impl<'a> PluginFactory<'a> for MyPlugin<'a> {
    fn new(host: &'a TypedHost<'a>) -> Self {
        MyPlugin { host }
    }
}

impl TlockNamespace<RpcErrorCode> for MyPlugin<'_> {
    fn ping(&self, message: String) -> Result<String, RpcErrorCode> {
        self.host.ping("Hello from plugin v2".to_string())?;

        let count = find_primes(10000);

        Ok(format!("Pong: message={}, primes={}", message, count))
    }
}

impl PluginNamespace<RpcErrorCode> for MyPlugin<'_> {
    fn name(&self) -> Result<String, RpcErrorCode> {
        Ok("Test Plugin".to_string())
    }

    fn version(&self) -> Result<String, RpcErrorCode> {
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

register_plugin!(MyPlugin<'_>);
