use std::{
    collections::HashMap,
    io::{BufRead, Write},
    sync::Arc,
};

use futures::{
    channel::oneshot::{self, Sender},
    lock::Mutex,
};
use runtime::yield_now;
use serde_json::Value;

use crate::{
    api::RequestHandler,
    rpc_message::{RpcError, RpcErrorCode, RpcErrorResponse, RpcMessage, RpcRequest, RpcResponse},
};

/// Single-threaded, concurrent-safe json-rpc transport layer
///
/// Use RefCells instead of having mutable functions since it lets
/// us run `call` from multiple different instances of the transport
/// at the same time, very helpful within the plugins.
pub struct JsonRpcTransport {
    pending: Mutex<HashMap<u64, Sender<RpcResponse>>>,
    reader: Mutex<Box<dyn BufRead + Send + Sync>>,
    writer: Mutex<Box<dyn Write + Send + Sync>>,
}

const JSON_RPC_VERSION: &str = "2.0";

impl JsonRpcTransport {
    pub fn new(
        reader: Box<dyn BufRead + Send + Sync>,
        writer: Box<dyn Write + Send + Sync>,
    ) -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            reader: Mutex::new(reader),
            writer: Mutex::new(writer),
        }
    }

    pub async fn call(
        &self,
        id: u64,
        method: &str,
        params: Value,
        handler: Option<Arc<dyn RequestHandler<RpcErrorCode>>>,
    ) -> Result<RpcResponse, RpcErrorCode> {
        let (tx, mut rx) = oneshot::channel();

        self.pending.lock().await.insert(id, tx);
        self.write_request(id, method, params).await?;

        loop {
            match rx.try_recv() {
                Ok(Some(msg)) => {
                    return Ok(msg);
                }
                Ok(None) => {}
                Err(_) => {
                    return Err(RpcErrorCode::InternalError);
                }
            }

            self.process_next_line(handler.clone()).await?;
        }
    }

    async fn write_request(
        &self,
        id: u64,
        method: &str,
        params: Value,
    ) -> Result<(), RpcErrorCode> {
        let msg = RpcMessage::RpcRequest(RpcRequest {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            method: method.to_string(),
            params,
        });
        self.write_message(&msg).await?;
        Ok(())
    }

    async fn write_message(&self, msg: &RpcMessage) -> Result<(), RpcErrorCode> {
        let serialized = serde_json::to_string(msg).map_err(|_| RpcErrorCode::InvalidParams)?;
        let msg = format!("{}\n", serialized);

        let mut writer = self.writer.lock().await;
        writer
            .write_all(msg.as_bytes())
            .map_err(|_| RpcErrorCode::InternalError)?;
        writer.flush().map_err(|_| RpcErrorCode::InternalError)?;
        Ok(())
    }

    pub async fn process_next_line(
        &self,
        handler: Option<Arc<dyn RequestHandler<RpcErrorCode>>>,
    ) -> Result<(), RpcErrorCode> {
        let line: String = self.next_line().await?;
        let message = match serde_json::from_str::<RpcMessage>(line.trim()) {
            Ok(msg) => msg,
            Err(_) => {
                println!("Failed to parse line: {}", line.trim());
                return Err(RpcErrorCode::ParseError);
            }
        };

        match message {
            RpcMessage::RpcResponse(resp) => {
                if let Some(tx) = self.pending.lock().await.remove(&resp.id) {
                    let _ = tx.send(resp);
                }
            }
            RpcMessage::RpcErrorResponse(err) => {
                return Err(err.error.code);
            }
            RpcMessage::RpcRequest(RpcRequest {
                jsonrpc: _,
                id,
                method,
                params,
            }) => {
                let handler = handler.ok_or(RpcErrorCode::MethodNotFound)?;
                match handler.handle(&method, params).await {
                    Ok(result) => {
                        let response = RpcMessage::RpcResponse(RpcResponse {
                            jsonrpc: JSON_RPC_VERSION.to_string(),
                            id,
                            result,
                        });
                        self.write_message(&response).await?;
                    }
                    Err(err) => {
                        let response = RpcMessage::RpcErrorResponse(RpcErrorResponse {
                            jsonrpc: JSON_RPC_VERSION.to_string(),
                            id,
                            error: RpcError {
                                code: err,
                                message: err.to_string(),
                            },
                        });
                        self.write_message(&response).await?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn next_line(&self) -> Result<String, RpcErrorCode> {
        let mut line = String::new();

        loop {
            match self.reader.lock().await.read_line(&mut line) {
                Ok(0) => {
                    // EOF
                    return Err(RpcErrorCode::InternalError);
                }
                Ok(_) => {
                    return Ok(line);
                }
                Err(e) => match e.kind() {
                    std::io::ErrorKind::WouldBlock => {
                        yield_now().await;
                        continue;
                    }
                    _ => return Err(RpcErrorCode::InternalError),
                },
            }
        }
    }
}
