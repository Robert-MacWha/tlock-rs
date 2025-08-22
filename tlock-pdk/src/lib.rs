// tlock-pdk
use std::{
    io::{BufRead, BufReader, Read, Write},
    sync::{
        atomic::AtomicBool,
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
};

use thiserror::Error;
use wasmer::{Module, Store};
use wasmer_wasi::{Pipe, WasiEnv};

pub struct PluginHost {
    send_tx: Sender<String>,
    recv_rx: Receiver<String>,
    stderr_rx: Receiver<String>,
    is_running: Arc<AtomicBool>,
}

#[derive(Error, Debug)]
pub enum SpawnError {
    #[error("compile error")]
    CompileError(#[from] wasmer::CompileError),
    #[error("unknown data store error")]
    Unknown,
}

#[derive(Error, Debug)]
pub enum PluginError {
    #[error("plugin not running")]
    NotRunning,
    #[error("send error")]
    SendError(#[from] std::sync::mpsc::SendError<String>),
    #[error("disconnected")]
    Disconnected,
}

impl PluginHost {
    pub fn spawn(program_name: &str, wasm_bytes: Vec<u8>) -> Result<Self, SpawnError> {
        let (send_tx, send_rx) = mpsc::channel::<String>();
        let (recv_tx, recv_rx) = mpsc::channel::<String>();
        let (stderr_tx, stderr_rx) = mpsc::channel::<String>();

        let is_running = Arc::new(AtomicBool::new(true));

        let store = Store::default();
        let module = Module::new(&store, wasm_bytes)?;
        let (stdin_tx, stdin_rx) = Pipe::channel();
        let (stdout_tx, stdout_rx) = Pipe::channel();
        let (stderr_tx_pipe, stderr_rx_pipe) = Pipe::channel();

        let env = WasiEnv::builder(program_name)
            .stdin(Box::new(stdin_rx))
            .stdout(Box::new(stdout_tx))
            .stderr(Box::new(stderr_tx_pipe));

        spawn_stdin_thread(stdin_tx, send_rx, is_running.clone());
        spawn_stdout_thread(stdout_rx, recv_tx, is_running.clone());
        spawn_stderr_thread(stderr_rx_pipe, stderr_tx, is_running.clone());
        spawn_runner_thread(module, store, env, is_running.clone());

        return Ok(Self {
            send_tx,
            recv_rx,
            stderr_rx,
            is_running,
        });
    }

    pub fn host_send(&self, msg: String) -> Result<(), PluginError> {
        if !self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(PluginError::NotRunning);
        }

        self.send_tx.send(msg)?;
        Ok(())
    }

    pub fn host_try_recv(&self) -> Result<Option<String>, PluginError> {
        if !self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(PluginError::NotRunning);
        }

        match self.recv_rx.try_recv() {
            Ok(msg) => Ok(Some(msg)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(PluginError::Disconnected),
        }
    }
}

fn spawn_stdin_thread(stdin_tx: Pipe, send_rx: Receiver<String>, is_running: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut stdin_tx = stdin_tx;
        for msg in send_rx {
            println!("Sending to plugin stdin: {}", msg);
            if let Err(e) = writeln!(stdin_tx, "{}", msg) {
                println!("Error writing to stdin: {}", e);
                break;
            }
            if let Err(e) = stdin_tx.flush() {
                println!("Error flushing stdin: {}", e);
                break;
            }
        }
        is_running.store(false, std::sync::atomic::Ordering::SeqCst);
        drop(stdin_tx);
    });
}

fn spawn_stdout_thread(stdout_rx: Pipe, recv_tx: Sender<String>, is_running: Arc<AtomicBool>) {
    thread::spawn(move || {
        let reader = BufReader::new(stdout_rx);
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    println!("Plugin stdout: {}", l);
                    if recv_tx.send(l).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    println!("Error reading from stdout: {}", e);
                    break;
                }
            }
        }

        is_running.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

fn spawn_stderr_thread(
    stderr_rx_pipe: Pipe,
    stderr_tx: Sender<String>,
    is_running: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let reader = BufReader::new(stderr_rx_pipe);
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    println!("Plugin stderr: {}", l);
                    if stderr_tx.send(l).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    println!("Error reading from stderr: {}", e);
                    break;
                }
            }
        }

        is_running.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

fn spawn_runner_thread(
    module: Module,
    store: Store,
    env: WasiEnvBuilder,
    is_running: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let mut store = store;
        match env.run_with_store(module, &mut store) {
            Ok(_) => {
                println!("Plugin has finished running");
            }
            Err(e) => {
                println!("Error running plugin: {}", e);
            }
        }
        is_running.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}
