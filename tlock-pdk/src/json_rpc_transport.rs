use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
};

use futures::channel::oneshot::{self, Sender};
use serde_json::Value;

use crate::transport::{RequestHandler, RpcMessage, Transport, TransportError};

pub struct JsonRpcTransport {
    writer: Box<dyn Write>,
    reader: BufReader<Box<dyn Read>>,
    next_id: u64,
    pending: HashMap<u64, Sender<RpcMessage>>,
}

impl JsonRpcTransport {
    pub fn new(writer: Box<dyn Write>, reader: Box<dyn Read>) -> Self {
        Self {
            writer,
            reader: BufReader::new(reader),
            next_id: 0,
            pending: HashMap::new(),
        }
    }
}

impl Transport for JsonRpcTransport {
    fn call(
        &mut self,
        method: &str,
        params: Value,
        handler: &mut dyn RequestHandler,
    ) -> Result<RpcMessage, TransportError> {
        let id = self.next_id;
        self.next_id += 1;

        let (tx, mut rx) = oneshot::channel();
        self.pending.insert(id, tx);
        self.write_request(id, method, params)?;

        loop {
            //? Check if we've received a response
            match rx.try_recv() {
                Ok(Some(msg)) => {
                    return Ok(msg);
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("Error receiving response: {}", e);
                    return Err(TransportError::ChannelClosed);
                }
            }

            //? Otherwise, try processing
            self.process_next_line(handler)?;
        }
    }
}

impl JsonRpcTransport {
    fn write_message(&mut self, msg: &RpcMessage) -> Result<(), TransportError> {
        let serialized = serde_json::to_string(msg)?;
        writeln!(self.writer, "{}", serialized)?;
        self.writer.flush()?;
        Ok(())
    }

    fn write_request(
        &mut self,
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
        self.write_message(&msg)?;
        Ok(())
    }

    pub fn process_next_line(
        &mut self,
        handler: &mut dyn RequestHandler,
    ) -> Result<(), TransportError> {
        let line = self.next_line()?;
        let message: RpcMessage = serde_json::from_str(line.trim())?;

        match message {
            RpcMessage::ResponseOk { id, .. } | RpcMessage::ResponseErr { id, .. } => {
                println!("Handling response for id: {}", id);
                if let Some(tx) = self.pending.remove(&id) {
                    let _ = tx.send(message);
                }
            }
            RpcMessage::Request {
                id, method, params, ..
            } => {
                println!("Handling request: {} with id: {}", method, id);

                match handler.handle(&method, params, self) {
                    Ok(result) => self.write_message(&RpcMessage::ResponseOk {
                        jsonrpc: "2.0".into(),
                        id,
                        result,
                    })?,
                    Err(e) => self.write_message(&RpcMessage::ResponseErr {
                        jsonrpc: "2.0".into(),
                        id,
                        error: Value::String(e.to_string()),
                    })?,
                };
            }
        }
        Ok(())
    }

    fn next_line(&mut self) -> Result<String, TransportError> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
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
