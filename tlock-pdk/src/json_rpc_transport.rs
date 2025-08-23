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
pub enum SendError {
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
    handler: Option<Arc<RequestHandler>>,
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

    pub fn set_handler<F>(&mut self, handler: F)
    where
        F: Fn(&str, Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>
            + Send
            + Sync
            + 'static,
    {
        self.handler = Some(Arc::new(handler));
    }

    pub fn call(&mut self, method: &str, params: Value) -> Result<Receiver<RpcMessage>, SendError> {
        let id = self.next_id;
        self.next_id += 1;

        let (resp_tx, resp_rx) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| SendError::LockError)?
            .insert(id, resp_tx);

        let msg = RpcMessage::Request {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let serialized = serde_json::to_string(&msg)?;
        let mut stdin = self.stdin_writer.lock().map_err(|_| SendError::LockError)?;
        writeln!(stdin, "{}", serialized)?;
        stdin.flush()?;

        Ok(resp_rx)
    }

    pub fn start_polling(&mut self, stdout_reader: PipeReader) -> Result<(), SendError> {
        if self.is_polling.load(Ordering::SeqCst) {
            return Ok(());
        }

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
        handler: Option<Arc<RequestHandler>>,
    ) {
        let mut buf_reader = BufReader::new(stdout_reader);
        let mut line = String::new();

        loop {
            line.clear();
            match buf_reader.read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {
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
        handler: &Option<Arc<RequestHandler>>,
    ) -> Result<(), SendError> {
        let message: RpcMessage = serde_json::from_str(line.trim())?;

        match message {
            RpcMessage::Request {
                id, method, params, ..
            } => {
                if let Some(handler) = handler {
                    match handler(&method, params) {
                        Ok(result) => {
                            let response = RpcMessage::Response {
                                jsonrpc: "2.0".to_string(),
                                id,
                                result: Some(result),
                                error: None,
                            };
                            let serialized = serde_json::to_string(&response)?;
                            let mut stdin =
                                stdin_writer.lock().map_err(|_| SendError::LockError)?;
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
                                stdin_writer.lock().map_err(|_| SendError::LockError)?;
                            writeln!(stdin, "{}", serialized)?;
                            stdin.flush()?;
                        }
                    }
                }
            }
            RpcMessage::Response { id, .. } => {
                if let Some(tx) = pending
                    .lock()
                    .map_err(|_| SendError::LockError)?
                    .remove(&id)
                {
                    let _ = tx.send(message);
                }
            }
        }

        Ok(())
    }
}

impl Transport for JsonRpcTransport {
    type Error = SendError;

    fn call(&mut self, method: &str, params: Value) -> Result<Receiver<RpcMessage>, Self::Error> {
        JsonRpcTransport::call(self, method, params)
    }

    fn set_handler<F>(&mut self, handler: F)
    where
        F: Fn(&str, Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>
            + Send
            + Sync
            + 'static,
    {
        JsonRpcTransport::set_handler(self, handler)
    }

    fn start_polling(&mut self, stdout_reader: PipeReader) -> Result<(), Self::Error> {
        JsonRpcTransport::start_polling(self, stdout_reader)
    }
}
