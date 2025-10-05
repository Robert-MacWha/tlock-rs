use std::{
    collections::HashMap,
    io::{BufRead, Write},
    sync::{Arc, atomic::AtomicU64},
};

use async_trait::async_trait;
use futures::{
    channel::oneshot::{self, Sender},
    lock::Mutex,
};
use runtime::yield_now;
use serde_json::Value;
use tracing::{trace, warn};

use crate::{
    api::{ApiError, RequestHandler},
    rpc_message::{RpcError, RpcErrorCode, RpcErrorResponse, RpcMessage, RpcRequest, RpcResponse},
};

#[async_trait]
pub trait Transport<E: ApiError> {
    async fn call(&self, method: &str, params: Value) -> Result<RpcResponse, E>;
}

/// Single-threaded, concurrent-safe json-rpc transport layer
///
/// Use RefCells instead of having mutable functions since it lets
/// us run `call` from multiple different instances of the transport
/// at the same time, very helpful within the plugins.
pub struct JsonRpcTransport {
    id: AtomicU64,
    pending: Mutex<HashMap<u64, Sender<RpcResponse>>>,
    reader: Mutex<Box<dyn BufRead + Send + Sync>>,
    writer: Mutex<Box<dyn Write + Send + Sync>>,
    handler: Option<Arc<dyn RequestHandler<RpcErrorCode>>>,
}

const JSON_RPC_VERSION: &str = "2.0";

impl JsonRpcTransport {
    pub fn new(
        reader: Box<dyn BufRead + Send + Sync>,
        writer: Box<dyn Write + Send + Sync>,
    ) -> Self {
        Self {
            id: AtomicU64::new(0),
            pending: Mutex::new(HashMap::new()),
            reader: Mutex::new(reader),
            writer: Mutex::new(writer),
            handler: None,
        }
    }

    pub fn with_handler(
        reader: Box<dyn BufRead + Send + Sync>,
        writer: Box<dyn Write + Send + Sync>,
        handler: Arc<dyn RequestHandler<RpcErrorCode>>,
    ) -> Self {
        Self {
            id: AtomicU64::new(0),
            pending: Mutex::new(HashMap::new()),
            reader: Mutex::new(reader),
            writer: Mutex::new(writer),
            handler: Some(handler),
        }
    }
}

#[async_trait]
impl Transport<RpcErrorCode> for JsonRpcTransport {
    /// Sends a json-rpc request and waits for the response. `Call` does not
    /// pump the reader, so you must call `process_next_line` in another task
    /// to read responses / handle incoming requests.
    async fn call(&self, method: &str, params: Value) -> Result<RpcResponse, RpcErrorCode> {
        let (tx, mut rx) = oneshot::channel();
        let id = self.id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.pending.lock().await.insert(id, tx);
        self.write_request(id, method, params).await?;

        //? Loop handles all incoming messages until we get a response for our call
        loop {
            match rx.try_recv() {
                Ok(Some(msg)) => {
                    return Ok(msg);
                }
                Ok(None) => {}
                Err(_) => {
                    warn!("Caller dropped");
                    return Err(RpcErrorCode::InternalError);
                }
            }

            self.process_next_line(self.handler.clone()).await?;
        }
    }
}

impl JsonRpcTransport {
    /// Processes a single request from the reader.  Can be used to manually
    /// pump the reader with a custom handler.
    pub async fn process_next_line(
        &self,
        handler: Option<Arc<dyn RequestHandler<RpcErrorCode>>>,
    ) -> Result<(), RpcErrorCode> {
        let line: String = self.next_line().await?;
        let message = match serde_json::from_str::<RpcMessage>(line.trim()) {
            Ok(msg) => msg,
            Err(_) => {
                warn!("Failed to parse line: {}", line.trim());
                return Err(RpcErrorCode::ParseError);
            }
        };

        self.process_message(message, handler).await
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
        let serialized = serde_json::to_string(msg).map_err(|_| {
            warn!("Failed to serialize message: {:?}", msg);
            RpcErrorCode::InternalError
        })?;
        let msg = format!("{}\n", serialized);

        trace!("JsonRpcTransport::write_message() - {}", msg.trim());
        let mut writer = self.writer.lock().await;
        writer.write_all(msg.as_bytes()).map_err(|_| {
            warn!("Failed to write message: {}", msg.trim());
            RpcErrorCode::InternalError
        })?;
        writer.flush().map_err(|_| {
            warn!("Failed to flush message: {}", msg.trim());
            RpcErrorCode::InternalError
        })?;
        Ok(())
    }

    async fn process_message(
        &self,
        message: RpcMessage,
        handler: Option<Arc<dyn RequestHandler<RpcErrorCode>>>,
    ) -> Result<(), RpcErrorCode> {
        match message.clone() {
            RpcMessage::RpcResponse(resp) => {
                if let Some(tx) = self.pending.lock().await.remove(&resp.id) {
                    let _ = tx.send(resp);
                }
            }
            RpcMessage::RpcErrorResponse(err) => {
                warn!(
                    "Received error response from plugin: {:?}",
                    err.error.message
                );
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

        // Loop until we get a line, awaiting if we would block
        //? Uses a loop here instead of an async reader since need this to
        //? also work in the plugin wasi environment. There it'll receive the
        //? `WouldBlock` error from the host's non-blocking pipe, but since
        //? it's just getting that through stdio it can't be async. And using
        //? async stdio won't work well because wasi.
        loop {
            match self.reader.lock().await.read_line(&mut line) {
                Ok(0) => {
                    warn!("EOF reached while reading line");
                    return Err(RpcErrorCode::InternalError);
                }
                Ok(_) => return Ok(line),
                Err(e) => match e.kind() {
                    std::io::ErrorKind::WouldBlock => {
                        yield_now().await;
                        continue;
                    }
                    _ => {
                        warn!("Failed to read line: {}", e);
                        return Err(RpcErrorCode::InternalError);
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use futures::executor::block_on;
    use std::io::{BufReader, Cursor};

    struct MockWriter {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl MockWriter {
        fn new(buffer: Arc<Mutex<Vec<u8>>>) -> Self {
            Self { buffer }
        }
    }

    impl Write for MockWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            block_on(async {
                self.buffer.lock().await.extend_from_slice(buf);
            });
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_response_received_and_pending_removed() {
        let response = RpcResponse {
            jsonrpc: "2.0".into(),
            id: 42,
            result: serde_json::json!({"success": true}),
        };
        let response_json = serde_json::to_string(&response).unwrap();

        let reader = Box::new(BufReader::new(Cursor::new(format!("{}\n", response_json))));
        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = Box::new(MockWriter::new(output_buffer));
        let transport = JsonRpcTransport::new(reader, writer);

        // Inject a fake pending request with the ID and a rx we control
        let (tx, mut rx) = oneshot::channel();
        transport.pending.lock().await.insert(42, tx);

        // Process the next line containing our expected response
        transport.process_next_line(None).await.unwrap();

        assert!(!transport.pending.lock().await.contains_key(&42));
        assert_eq!(rx.try_recv().unwrap().unwrap(), response);
    }
}
