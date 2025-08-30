use std::{
    cell::RefCell,
    collections::HashMap,
    io::{BufRead, Write},
};

use futures::channel::oneshot::{self, Sender};
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
    pending: RefCell<HashMap<u64, Sender<RpcResponse>>>,
    reader: RefCell<Box<dyn BufRead>>,
    writer: RefCell<Box<dyn Write>>,
}

const JSON_RPC_VERSION: &str = "2.0";

impl JsonRpcTransport {
    pub fn new(reader: Box<dyn BufRead>, writer: Box<dyn Write>) -> Self {
        Self {
            pending: RefCell::new(HashMap::new()),
            reader: RefCell::new(reader),
            writer: RefCell::new(writer),
        }
    }

    pub fn call(
        &self,
        id: u64,
        method: &str,
        params: Value,
        handler: Option<&dyn RequestHandler<RpcErrorCode>>,
    ) -> Result<RpcResponse, RpcErrorCode> {
        let (tx, mut rx) = oneshot::channel();
        self.pending.borrow_mut().insert(id, tx);
        self.write_request(id, method, params)?;

        loop {
            //? Check if we've received a response
            match rx.try_recv() {
                Ok(Some(msg)) => {
                    return Ok(msg);
                }
                Ok(None) => {}
                Err(_) => {
                    return Err(RpcErrorCode::InternalError);
                }
            }

            //? Otherwise, try processing
            self.process_next_line(handler)?;
        }
    }

    fn write_request(&self, id: u64, method: &str, params: Value) -> Result<(), RpcErrorCode> {
        let msg = RpcMessage::RpcRequest(RpcRequest {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            method: method.to_string(),
            params,
        });
        self.write_message(&msg)?;
        Ok(())
    }

    fn write_message(&self, msg: &RpcMessage) -> Result<(), RpcErrorCode> {
        let serialized = serde_json::to_string(msg).map_err(|_| RpcErrorCode::InvalidParams)?;

        let mut writer = self.writer.borrow_mut();
        writeln!(writer, "{}", serialized).map_err(|_| RpcErrorCode::InternalError)?;
        writer.flush().map_err(|_| RpcErrorCode::InternalError)?;
        Ok(())
    }

    pub fn process_next_line(
        &self,
        handler: Option<&dyn RequestHandler<RpcErrorCode>>,
    ) -> Result<(), RpcErrorCode> {
        let line = self.next_line()?;
        let message = serde_json::from_str(line.trim()).map_err(|_| RpcErrorCode::ParseError)?;

        match message {
            RpcMessage::RpcResponse(resp) => {
                if let Some(tx) = self.pending.borrow_mut().remove(&resp.id) {
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

                match handler.handle(&method, params) {
                    Ok(result) => {
                        let response = RpcMessage::RpcResponse(RpcResponse {
                            jsonrpc: JSON_RPC_VERSION.to_string(),
                            id,
                            result,
                        });
                        self.write_message(&response)?;
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
                        self.write_message(&response)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn next_line(&self) -> Result<String, RpcErrorCode> {
        let mut line = String::new();
        match self.reader.borrow_mut().read_line(&mut line) {
            Ok(0) => return Err(RpcErrorCode::InternalError),
            Ok(_) => {
                return Ok(line);
            }
            Err(_) => {
                return Err(RpcErrorCode::InternalError);
            }
        }
    }
}
