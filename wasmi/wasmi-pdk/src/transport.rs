use std::{
    collections::HashMap,
    io::{BufRead, Write},
    sync::Arc,
};

use futures::{
    channel::oneshot::{self, Sender},
    lock::Mutex,
};
use log::warn;
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

    /// Sends a json-rpc request and waits for the response
    /// If a handler is provided, it will be used to handle incoming requests
    /// while waiting for the response.
    ///
    /// - When calling from a plugin: A given plugin instance will only ever
    ///     receive a single request. Because of this handler should be None.
    /// - When calling from the host: The host may receive multiple requests from
    ///     a plugin before it returns a response. Handler should be set to Some(...).
    ///
    /// TODO: Make the above explicit in the type system
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

        //? Loop handles all incoming messages until we get a response for our call
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

    /// Processes a single request from the reader. For the plugin, this can be used
    /// to process the single incoming request from the host. For the host this doesn't
    /// really matter, or can be used to step through requests one by one.
    ///
    /// TODO: Make the above explicit in the type system
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
        let serialized = serde_json::to_string(msg).map_err(|_| RpcErrorCode::InvalidParams)?;
        let msg = format!("{}\n", serialized);

        let mut writer = self.writer.lock().await;
        writer
            .write_all(msg.as_bytes())
            .map_err(|_| RpcErrorCode::InternalError)?;
        writer.flush().map_err(|_| RpcErrorCode::InternalError)?;
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

        return Ok(());
    }

    async fn next_line(&self) -> Result<String, RpcErrorCode> {
        let mut line = String::new();

        loop {
            match self.reader.lock().await.read_line(&mut line) {
                Ok(0) => return Err(RpcErrorCode::InternalError), // EOF
                Ok(_) => return Ok(line),
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
