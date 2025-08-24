use std::{io::BufReader, sync::atomic::AtomicU64};

use serde_json::Value;
use thiserror::Error;
use tlock_pdk::{
    json_rpc_transport::{JsonRpcTransport, JsonRpcTransportError},
    request_handler::RequestHandler,
    transport::RpcMessage,
};

use crate::plugin_instance::{PluginInstance, SpawnError};

/// Plugin is an async-capable instance of a plugin
pub struct Plugin<'a> {
    wasm_bytes: Vec<u8>,
    id: AtomicU64,
    handler: &'a dyn RequestHandler,
}

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("spawn error")]
    SpawnError(#[from] SpawnError),
    #[error("transport error")]
    JsonRpcTransportError(#[from] JsonRpcTransportError),
}

impl<'a> Plugin<'a> {
    pub fn new(wasm_bytes: Vec<u8>, handler: &'a dyn RequestHandler) -> Self {
        Plugin {
            wasm_bytes,
            id: AtomicU64::new(0),
            handler,
        }
    }

    pub fn call(&self, method: &str, params: Value) -> Result<RpcMessage, PluginError> {
        let (_instance, stdin_writer, stdout_reader) =
            PluginInstance::new(self.wasm_bytes.clone())?;

        let id = self.id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let buf_reader = BufReader::new(stdout_reader);
        let transport = JsonRpcTransport::new(Box::new(buf_reader), Box::new(stdin_writer));
        let res = transport.call(id, method, params, Some(self.handler))?;
        return Ok(res);
    }
}
