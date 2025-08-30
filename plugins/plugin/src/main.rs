use std::io::{self, BufReader};

use tlock_pdk::{
    api::{Plugin, PluginApi, PluginNamespace, TlockNamespace},
    rpc_message::RpcErrorCode,
    transport::json_rpc_transport::JsonRpcTransport,
    typed_host::TypedHost,
};
struct MyPlugin<'a> {
    host: &'a TypedHost<'a>,
}

fn main() {
    let writer = io::stdout();
    let reader = io::stdin();
    let reader = BufReader::new(reader);

    let transport = JsonRpcTransport::new(Box::new(reader), Box::new(writer));
    let host = TypedHost::new(&transport);

    let plugin = MyPlugin { host: &host };
    let plugin = Plugin(plugin);

    transport.process_next_line(Some(&plugin)).unwrap();
}

impl PluginApi<RpcErrorCode> for MyPlugin<'_> {}

impl TlockNamespace<RpcErrorCode> for MyPlugin<'_> {
    fn ping(&self, message: String) -> Result<String, RpcErrorCode> {
        self.host.ping("Hello from plugin".to_string())?;
        Ok(format!("Pong: message={}", message))
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
