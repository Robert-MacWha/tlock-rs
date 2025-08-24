use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    sync::{Arc, Mutex, atomic::AtomicU64},
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
}

/// Single-threaded, concurrent-safe json-rpc transport layer
pub struct JsonRpcTransport {
    next_id: AtomicU64,
    pending: Arc<Mutex<HashMap<u64, Sender<RpcMessage>>>>,
}

impl JsonRpcTransport {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(0),
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn call(
        &self,
        reader: impl Read,
        writer: &mut dyn Write,
        method: &str,
        params: Value,
        handler: &dyn RequestHandler,
    ) -> Result<RpcMessage, JsonRpcTransportError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let (tx, mut rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        self.write_request(writer, id, method, params)?;

        println!("Waiting for response to id: {}", id);
        let mut reader = BufReader::new(reader);
        loop {
            //? Check if we've received a response
            match rx.try_recv() {
                Ok(Some(msg)) => {
                    println!("Received response for id: {}", id);
                    return Ok(msg);
                }
                Ok(None) => {}
                Err(e) => {
                    println!("Error receiving response: {}", e);
                    return Err(JsonRpcTransportError::ChannelClosed);
                }
            }

            //? Otherwise, try processing
            self.process_next_line(&mut reader, writer, handler)?;
        }
    }

    fn write_request(
        &self,
        writer: &mut dyn Write,
        id: u64,
        method: &str,
        params: Value,
    ) -> Result<(), JsonRpcTransportError> {
        println!("Sending request id: {} method: {}", id, method);
        let msg = RpcMessage::Request {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };
        self.write_message(writer, &msg)?;
        Ok(())
    }

    fn write_message(
        &self,
        writer: &mut dyn Write,
        msg: &RpcMessage,
    ) -> Result<(), JsonRpcTransportError> {
        let serialized = serde_json::to_string(msg).map_err(|e| {
            JsonRpcTransportError::Serde(e, format!("Failed to serialize message: {:?}", msg))
        })?;
        println!("Sending message: {}", serialized);
        writeln!(writer, "{}", serialized)?;
        writer.flush()?;
        Ok(())
    }

    pub fn process_next_line(
        &self,
        reader: &mut dyn BufRead,
        writer: &mut dyn Write,
        handler: &dyn RequestHandler,
    ) -> Result<(), JsonRpcTransportError> {
        let line = self.next_line(reader)?;
        let message = serde_json::from_str(line.trim()).map_err(|e| {
            JsonRpcTransportError::Serde(e, format!("Failed to deserialize line: {}", line.trim()))
        })?;

        match message {
            RpcMessage::ResponseOk { id, .. } | RpcMessage::ResponseErr { id, .. } => {
                println!("Handling response for id: {}", id);
                if let Some(tx) = self.pending.lock().unwrap().remove(&id) {
                    let _ = tx.send(message);
                }
            }
            RpcMessage::Request {
                id, method, params, ..
            } => {
                println!("Handling request: {} with id: {}", method, id);

                match handler.handle(&method, params) {
                    Ok(result) => {
                        let response = RpcMessage::ResponseOk {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result,
                        };
                        self.write_message(writer, &response)?;
                    }
                    Err(err) => {
                        let response = RpcMessage::ResponseErr {
                            jsonrpc: "2.0".to_string(),
                            id,
                            error: err.to_string(),
                        };
                        self.write_message(writer, &response)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn next_line(&self, reader: &mut dyn BufRead) -> Result<String, JsonRpcTransportError> {
        let mut line = String::new();
        match reader.read_line(&mut line) {
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
