use std::{
    io::{PipeReader, PipeWriter, pipe},
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self},
    },
    thread::{self},
};

use serde_json::Value;
use thiserror::Error;
use wasmi::{Config, Engine, Func, Linker, Module, Store};
use wasmi_wasi::{
    WasiCtx, WasiCtxBuilder,
    wasi_common::pipe::{ReadPipe, WritePipe},
};

use crate::{
    json_rpc_transport::JsonRpcTransport,
    transport::{RequestHandler, RpcMessage, Transport, TransportError},
};

/// PluginHost manages a plugin's lifecycle and io, implementing
/// std::io::Read and std::io::Write
pub struct Plugin {
    transport: JsonRpcTransport,
    is_running: Arc<AtomicBool>,
}

#[derive(Error, Debug)]
pub enum SpawnError {
    #[error("io error")]
    IoError(#[from] std::io::Error),
    #[error("start not found")]
    StartNotFound,
    #[error("wasmi error")]
    WasmiError(#[from] wasmi::Error),
    #[error("wasi error")]
    WasiError(#[from] wasmi_wasi::Error),
    #[error("host trap")]
    HostTrap(wasmi::ResumableCallHostTrap),
    #[error("transport error")]
    TransportError(#[from] TransportError),
}

#[derive(Error, Debug)]
pub enum SendError {
    #[error("not running")]
    NotRunning,
    #[error("send error")]
    SendError(#[from] mpsc::SendError<String>),
    #[error("io error")]
    IoError(#[from] std::io::Error),
    #[error("lock error")]
    LockError,
}

#[derive(Error, Debug)]
pub enum RecvError {
    #[error("not running")]
    NotRunning,
    #[error("recive error")]
    RecvError(#[from] mpsc::RecvError),
    #[error("try receive error")]
    TryRecvError(#[from] mpsc::TryRecvError),
}

const MAX_FUEL: u64 = 1_000_000;

impl Plugin {
    /// Spawns the wasi plugin in a new thread
    pub fn new(wasm_bytes: Vec<u8>) -> Result<Self, SpawnError> {
        let is_running = Arc::new(AtomicBool::new(true));

        // Setup pipes
        let (stdin_reader, stdin_writer) = pipe()?;
        let (stdout_reader, stdout_writer) = pipe()?;

        start_plugin(wasm_bytes, &is_running, stdin_reader, stdout_writer)?;

        let transport = JsonRpcTransport::new(Box::new(stdin_writer), Box::new(stdout_reader));

        Ok(Plugin {
            transport,
            is_running,
        })
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn kill(&mut self) {
        self.is_running
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Transport for Plugin {
    fn call(
        &mut self,
        method: &str,
        params: Value,
        handler: &mut dyn RequestHandler,
    ) -> Result<RpcMessage, TransportError> {
        if !self.is_running() {
            return Err(TransportError::ChannelClosed);
        }
        self.transport.call(method, params, handler)
    }
}

fn start_plugin(
    wasm_bytes: Vec<u8>,
    is_running: &Arc<AtomicBool>,
    stdin_reader: PipeReader,
    stdout_writer: PipeWriter,
) -> Result<(), SpawnError> {
    let stdin_pipe = ReadPipe::new(stdin_reader);
    let stdout_pipe = WritePipe::new(stdout_writer);

    let mut config = Config::default();
    config.consume_fuel(true);
    // https://github.com/wasmi-labs/wasmi/issues/1647
    config.compilation_mode(wasmi::CompilationMode::Eager);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, wasm_bytes)?;

    let mut linker = <Linker<WasiCtx>>::new(&engine);
    let wasi = WasiCtxBuilder::new()
        .stdin(Box::new(stdin_pipe))
        .stdout(Box::new(stdout_pipe))
        .build();

    let mut store = Store::new(&engine, wasi);
    wasmi_wasi::add_to_linker(&mut linker, |ctx| ctx)?;

    let instance = linker.instantiate_and_start(&mut store, &module)?;
    let start_func = instance
        .get_func(&store, "_start")
        .ok_or(SpawnError::StartNotFound)?;

    thread::spawn({
        let is_running = is_running.clone();
        move || match run_wasm(store, start_func, is_running.clone()) {
            Ok(_) => {
                is_running.store(false, std::sync::atomic::Ordering::SeqCst);
            }
            Err(e) => {
                is_running.store(false, std::sync::atomic::Ordering::SeqCst);
                eprintln!("Thread died: {:?}", e);
            }
        }
    });
    Ok(())
}

/// run_wasm manages the plugin's lifecycle. Essentially - because
/// wasmi doesn't support any plugin intercept or halting, we need some manual
/// way of interrupting the plugin every so often to check if it's been killed,
/// and resuming it if it hasn't. Here I do that by setting a low fuel limit,
/// catching the out-of-fuel condition and resuming the plugin when it's not killed.
///  
/// TODO: When wasmi implements plugin interception (like interrupt_handle), switch
/// to that.
/// https://docs.rs/wasmtime/0.27.0/wasmtime/struct.Store.html#method.interrupt_handle
fn run_wasm(
    mut store: Store<WasiCtx>,
    start_func: Func,
    is_running: Arc<AtomicBool>,
) -> Result<(), SpawnError> {
    store.set_fuel(MAX_FUEL).unwrap();
    let mut resumable = start_func.call_resumable(&mut store, &[], &mut [])?;

    loop {
        match resumable {
            wasmi::ResumableCall::Finished => return Ok(()),
            wasmi::ResumableCall::HostTrap(trap) => {
                println!("Host trap: {:?}", trap);
                return Err(SpawnError::HostTrap(trap));
            }
            wasmi::ResumableCall::OutOfFuel(out_of_fuel) => {
                println!("Out of fuel, checking if still running...");
                if !is_running.load(std::sync::atomic::Ordering::SeqCst) {
                    return Ok(());
                }

                println!("Still running, resuming...");

                let required = out_of_fuel.required_fuel();
                let top_up = required.max(MAX_FUEL);
                println!("Topping up fuel by {}", top_up);
                store.set_fuel(top_up).unwrap();

                match out_of_fuel.resume(&mut store, &mut []) {
                    Ok(next) => resumable = next,
                    Err(e) => return Err(SpawnError::WasmiError(e)),
                }
            }
        }
    }
}
