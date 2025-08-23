use std::{
    collections::HashMap,
    io::{BufRead, BufReader, PipeReader, PipeWriter, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self},
};

use serde_json::Value;
use thiserror::Error;

use crate::transport::{RequestHandler, RpcMessage, Transport};

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("serde error")]
    Serde(#[from] serde_json::Error),
    #[error("lock error")]
    LockError,
}

pub struct JsonRpcTransport {
    stdin_writer: Arc<Mutex<PipeWriter>>,
    next_id: u64,
    pending: Arc<Mutex<HashMap<u64, Sender<RpcMessage>>>>,
    handler: Option<Arc<dyn RequestHandler>>,
    is_polling: Arc<AtomicBool>,
}

impl JsonRpcTransport {
    pub fn new(stdin_writer: Arc<Mutex<PipeWriter>>) -> Self {
        Self {
            stdin_writer,
            next_id: 0,
            pending: Arc::new(Mutex::new(HashMap::new())),
            handler: None,
            is_polling: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Transport for JsonRpcTransport {
    type Error = TransportError;

    fn set_handler(&mut self, handler: Arc<dyn RequestHandler>) {
        self.handler = Some(handler);
    }

    fn call(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Receiver<RpcMessage>, TransportError> {
        let id = self.next_id;
        self.next_id += 1;

        let (resp_tx, resp_rx) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| TransportError::LockError)?
            .insert(id, resp_tx);

        let msg = RpcMessage::Request {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let serialized = serde_json::to_string(&msg)?;
        let mut stdin = self
            .stdin_writer
            .lock()
            .map_err(|_| TransportError::LockError)?;
        println!("Sending message: {}", serialized);
        writeln!(stdin, "{}", serialized)?;
        stdin.flush()?;

        Ok(resp_rx)
    }
}

impl JsonRpcTransport {
    pub fn start_polling(&self, stdout_reader: PipeReader) -> Result<(), TransportError> {
        if self.is_polling.load(Ordering::SeqCst) {
            return Ok(());
        }

        println!("Starting polling thread...");

        self.is_polling.store(true, Ordering::SeqCst);

        let stdin_writer = self.stdin_writer.clone();
        let pending = self.pending.clone();
        let handler = self.handler.clone();

        thread::spawn(move || {
            Self::polling_thread(stdout_reader, stdin_writer, pending, handler);
        });

        Ok(())
    }

    fn polling_thread(
        stdout_reader: PipeReader,
        stdin_writer: Arc<Mutex<PipeWriter>>,
        pending: Arc<Mutex<HashMap<u64, Sender<RpcMessage>>>>,
        handler: Option<Arc<dyn RequestHandler>>,
    ) {
        let mut buf_reader = BufReader::new(stdout_reader);
        let mut line = String::new();

        loop {
            line.clear();
            println!("Waiting for message...");
            match buf_reader.read_line(&mut line) {
                Ok(0) => {
                    println!("EOF reached, stopping polling thread.");
                    break;
                }
                Ok(_) => {
                    println!("Received line: {}", line.trim());
                    if line.trim().is_empty() {
                        continue;
                    }

                    if let Err(e) = Self::handle_message(&line, &stdin_writer, &pending, &handler) {
                        eprintln!("Error handling message: {:?}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Error reading from stdout: {:?}", e);
                    break;
                }
            };
        }
    }

    fn handle_message(
        line: &str,
        stdin_writer: &Arc<Mutex<PipeWriter>>,
        pending: &Arc<Mutex<HashMap<u64, Sender<RpcMessage>>>>,
        handler: &Option<Arc<dyn RequestHandler>>,
    ) -> Result<(), TransportError> {
        let message: RpcMessage = serde_json::from_str(line.trim())?;

        match message {
            RpcMessage::Response { id, .. } => {
                println!("Handling response for id: {}", id);
                if let Some(tx) = pending
                    .lock()
                    .map_err(|_| TransportError::LockError)?
                    .remove(&id)
                {
                    let _ = tx.send(message);
                }
            }
            RpcMessage::Request {
                id, method, params, ..
            } => {
                println!("Handling request: {} with id: {}", method, id);
                if let Some(handler) = handler {
                    match handler.handle(&method, params) {
                        Ok(result) => {
                            let response = RpcMessage::Response {
                                jsonrpc: "2.0".to_string(),
                                id,
                                result: Some(result),
                                error: None,
                            };
                            let serialized = serde_json::to_string(&response)?;
                            let mut stdin =
                                stdin_writer.lock().map_err(|_| TransportError::LockError)?;
                            writeln!(stdin, "{}", serialized)?;
                            stdin.flush()?;
                        }
                        Err(e) => {
                            let error_response = RpcMessage::Response {
                                jsonrpc: "2.0".to_string(),
                                id,
                                result: None,
                                error: Some(Value::String(e.to_string())),
                            };
                            let serialized = serde_json::to_string(&error_response)?;
                            let mut stdin =
                                stdin_writer.lock().map_err(|_| TransportError::LockError)?;
                            writeln!(stdin, "{}", serialized)?;
                            stdin.flush()?;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
