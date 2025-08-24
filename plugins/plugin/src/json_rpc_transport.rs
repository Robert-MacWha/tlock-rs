use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    sync::{Arc, Mutex, atomic::AtomicU64},
};

use futures::channel::oneshot::{self, Sender};
use serde_json::Value;

use crate::{
    request_handler::RequestHandler,
    transport::{RpcMessage, Transport, TransportError},
};

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
}

impl Transport for JsonRpcTransport {
    type Error = TransportError;

    fn call(
        &self,
        reader: impl Read,
        writer: &mut dyn Write,
        method: &str,
        params: Value,
        handler: &dyn RequestHandler,
    ) -> Result<RpcMessage, TransportError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let (tx, mut rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        self.write_request(writer, id, method, params)?;

        let mut reader = BufReader::new(reader);
        loop {
            //? Check if we've received a response
            match rx.try_recv() {
                Ok(Some(msg)) => {
                    return Ok(msg);
                }
                Ok(None) => {}
                Err(e) => {
                    return Err(TransportError::ChannelClosed);
                }
            }

            //? Otherwise, try processing
            self.process_next_line(&mut reader, writer, handler)?;
        }
    }
}

impl JsonRpcTransport {
    fn write_request(
        &self,
        writer: &mut dyn Write,
        id: u64,
        method: &str,
        params: Value,
    ) -> Result<(), TransportError> {
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
    ) -> Result<(), TransportError> {
        let serialized = serde_json::to_string(msg)?;
        writeln!(writer, "{}", serialized)?;
        writer.flush()?;
        Ok(())
    }

    pub fn process_next_line(
        &self,
        reader: &mut dyn BufRead,
        writer: &mut dyn Write,
        handler: &dyn RequestHandler,
    ) -> Result<(), TransportError> {
        let line = self.next_line(reader)?;
        let message: RpcMessage = serde_json::from_str(line.trim())?;

        match message {
            RpcMessage::ResponseOk { id, .. } | RpcMessage::ResponseErr { id, .. } => {
                if let Some(tx) = self.pending.lock().unwrap().remove(&id) {
                    let _ = tx.send(message);
                }
            }
            RpcMessage::Request {
                id, method, params, ..
            } => match handler.handle(&method, params) {
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
            },
        }
        Ok(())
    }

    fn next_line(&self, reader: &mut dyn BufRead) -> Result<String, TransportError> {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => return Err(TransportError::EOF),
            Ok(_) => {
                return Ok(line);
            }
            Err(e) => {
                return Err(TransportError::Io(e));
            }
        }
    }
}
