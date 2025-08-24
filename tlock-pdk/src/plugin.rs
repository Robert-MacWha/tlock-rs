use serde_json::Value;
use thiserror::Error;

use crate::{
    json_rpc_transport::JsonRpcTransport,
    plugin_instance::{PluginInstance, SpawnError},
    request_handler::RequestHandler,
    transport::{RpcMessage, Transport, TransportError},
};

/// Plugin is an async-capable instance of a plugin
pub struct Plugin<'a> {
    wasm_bytes: Vec<u8>,
    handler: &'a dyn RequestHandler,
    transport: JsonRpcTransport,
}

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("spawn error")]
    SpawnError(#[from] SpawnError),
    #[error("transport error")]
    TransportError(#[from] TransportError),
}

impl<'a> Plugin<'a> {
    pub fn new(wasm_bytes: Vec<u8>, handler: &'a dyn RequestHandler) -> Self {
        Plugin {
            wasm_bytes,
            handler,
            transport: JsonRpcTransport::new(),
        }
    }

    pub fn call(&self, method: &str, params: Value) -> Result<RpcMessage, PluginError> {
        let (_instance, stdin_writer, stdout_reader) =
            PluginInstance::new(self.wasm_bytes.clone())?;

        let reader = Box::new(stdout_reader);
        let writer = &mut Box::new(stdin_writer);

        let res = self
            .transport
            .call(reader, writer, method, params, self.handler)?;
        return Ok(res);
    }
}
