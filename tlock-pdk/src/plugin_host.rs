use std::{
    io::{PipeReader, PipeWriter, pipe},
    sync::{
        Arc, Mutex,
        atomic::AtomicBool,
        mpsc::{self},
    },
    thread::{self},
};

use thiserror::Error;
use wasmi::{Config, Engine, Func, Linker, Module, Store};
use wasmi_wasi::{
    WasiCtx, WasiCtxBuilder,
    wasi_common::pipe::{ReadPipe, WritePipe},
};


/// PluginHost manages a plugin's lifecycle and io, implementing
/// std::io::Read and std::io::Write
pub struct PluginHost {
    stdin_writer: Option<Arc<Mutex<PipeWriter>>>,
    stdout_reader: Option<Arc<Mutex<PipeReader>>>,
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

const MAX_FUEL: u64 = 10_000;

impl PluginHost {
    /// Spawns the wasi plugin in a new thread
    pub fn spawn(wasm_bytes: Vec<u8>) -> Result<Self, SpawnError> {
        let is_running = Arc::new(AtomicBool::new(true));

        let (stdin_reader, stdin_writer) = pipe()?;
        let (stdout_reader, stdout_writer) = pipe()?;

        let stdin_writer = Arc::new(Mutex::new(stdin_writer));
        let stdout_reader = Arc::new(Mutex::new(stdout_reader));

        let stdin_pipe = ReadPipe::new(stdin_reader);
        let stdout_pipe = WritePipe::new(stdout_writer);

        let config = Config::default();
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
            move || match handle_start_thread(store, start_func, is_running.clone()) {
                Ok(_) => {
                    is_running.store(false, std::sync::atomic::Ordering::SeqCst);
                }
                Err(e) => {
                    is_running.store(false, std::sync::atomic::Ordering::SeqCst);
                    eprintln!("Thread died: {:?}", e);
                }
            }
        });

        Ok(PluginHost {
            stdin_writer: Some(stdin_writer),
            stdout_reader: Some(stdout_reader),
            is_running,
        })
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn kill(mut self) {
        self.is_running
            .store(false, std::sync::atomic::Ordering::SeqCst);

        if let Some(writer) = self.stdin_writer.take() {
            drop(writer);
        }

        if let Some(rx) = self.stdout_reader.take() {
            drop(rx);
        }
    }
}

/// handle_start_thread manages the plugin's lifecycle. Essentially - because
/// wasmi doesn't support any plugin intercept or halting, we need some manual
/// way of interrupting the plugin every so often to check if it's been killed,
/// and resuming it if it hasn't. Here I do that by setting a low fuel limit,
/// catching the out-of-fuel condition and resuming the plugin when it's not killed.
///  
/// TODO: When wasmi implements plugin interception (like interrupt_handle), switch
/// to that.
/// https://docs.rs/wasmtime/0.27.0/wasmtime/struct.Store.html#method.interrupt_handle
fn handle_start_thread(
    mut store: Store<WasiCtx>,
    start_func: Func,
    is_running: Arc<AtomicBool>,
) -> Result<(), SpawnError> {
    store.set_fuel(MAX_FUEL).unwrap();
    let mut resumable = start_func.call_resumable(&mut store, &[], &mut [])?;

    loop {
        match resumable {
            wasmi::ResumableCall::Finished => return Ok(()),
            wasmi::ResumableCall::HostTrap(trap) => return Err(SpawnError::HostTrap(trap)),
            wasmi::ResumableCall::OutOfFuel(out_of_fuel) => {
                if !is_running.load(std::sync::atomic::Ordering::SeqCst) {
                    return Ok(());
                }

                let required = out_of_fuel.required_fuel();
                let top_up = required.max(MAX_FUEL);
                store.set_fuel(top_up).unwrap();

                match out_of_fuel.resume(&mut store, &mut []) {
                    Ok(next) => resumable = next,
                    Err(e) => return Err(SpawnError::WasmiError(e)),
                }
            }
        }
    }
}
