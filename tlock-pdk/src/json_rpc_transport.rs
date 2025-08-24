use std::{
    cell::RefCell,
    collections::HashMap,
    io::{BufRead, Write},
    rc::Rc,
};

use futures::channel::oneshot::{self, Sender};
use serde_json::Value;
use thiserror::Error;

use crate::{request_handler::RequestHandler, transport::RpcMessage};

#[derive(Error, Debug)]
pub enum JsonRpcTransportError {
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("deserialize error")]
    Serde(serde_json::Error, String),
    #[error("end of file")]
    EOF,
    #[error("response channel closed")]
    ChannelClosed,
    #[error("request timed out")]
    Timeout,
    #[error("no handler provided")]
    NoHandler,
}

/// Single-threaded, concurrent-safe json-rpc transport layer
pub struct JsonRpcTransport {
    pending: RefCell<HashMap<u64, Sender<RpcMessage>>>,
    reader: RefCell<Box<dyn BufRead>>,
    writer: RefCell<Box<dyn Write>>,
}

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
        handler: Option<&dyn RequestHandler>,
    ) -> Result<RpcMessage, JsonRpcTransportError> {
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
                    return Err(JsonRpcTransportError::ChannelClosed);
                }
            }

            //? Otherwise, try processing
            self.process_next_line(handler)?;
        }
    }

    fn write_request(
        &self,
        id: u64,
        method: &str,
        params: Value,
    ) -> Result<(), JsonRpcTransportError> {
        let msg = RpcMessage::Request {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };
        self.write_message(&msg)?;
        Ok(())
    }

    fn write_message(&self, msg: &RpcMessage) -> Result<(), JsonRpcTransportError> {
        let serialized = serde_json::to_string(msg).map_err(|e| {
            JsonRpcTransportError::Serde(e, format!("Failed to serialize message: {:?}", msg))
        })?;
        writeln!(self.writer.borrow_mut(), "{}", serialized)?;
        self.writer.borrow_mut().flush()?;
        Ok(())
    }

    pub fn process_next_line(
        &self,
        handler: Option<&dyn RequestHandler>,
    ) -> Result<(), JsonRpcTransportError> {
        let line = self.next_line()?;
        let message = serde_json::from_str(line.trim()).map_err(|e| {
            JsonRpcTransportError::Serde(e, format!("Failed to deserialize line: {}", line.trim()))
        })?;

        match message {
            RpcMessage::ResponseOk { id, .. } | RpcMessage::ResponseErr { id, .. } => {
                if let Some(tx) = self.pending.borrow_mut().remove(&id) {
                    let _ = tx.send(message);
                }
            }
            RpcMessage::Request {
                id, method, params, ..
            } => {
                let handler = handler.ok_or(JsonRpcTransportError::NoHandler)?;

                match handler.handle(&method, params) {
                    Ok(result) => {
                        let response = RpcMessage::ResponseOk {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result,
                        };
                        self.write_message(&response)?;
                    }
                    Err(err) => {
                        let response = RpcMessage::ResponseErr {
                            jsonrpc: "2.0".to_string(),
                            id,
                            error: err.to_string(),
                        };
                        self.write_message(&response)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn next_line(&self) -> Result<String, JsonRpcTransportError> {
        let mut line = String::new();
        match self.reader.borrow_mut().read_line(&mut line) {
            Ok(0) => return Err(JsonRpcTransportError::EOF),
            Ok(_) => {
                return Ok(line);
            }
            Err(e) => {
                return Err(JsonRpcTransportError::Io(e));
            }
        }
    }
}
