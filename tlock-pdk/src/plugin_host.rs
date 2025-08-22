use std::{
    io::{BufRead, BufReader, BufWriter, PipeWriter, Write, pipe},
    sync::{
        Arc, Mutex,
        atomic::AtomicBool,
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
};

use thiserror::Error;
use wasmi::{Config, Engine, Linker, Module, Store};
use wasmi_wasi::{
    WasiCtx, WasiCtxBuilder,
    wasi_common::pipe::{ReadPipe, WritePipe},
};

pub struct PluginHost {
    stdin_writer: Option<Arc<Mutex<BufWriter<PipeWriter>>>>,
    stdout_receiver: Option<Receiver<String>>,
    is_running: Arc<AtomicBool>,

    _wasi_handle: JoinHandle<()>,
    _output_handle: JoinHandle<()>,
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
    pub fn spawn(wasm_bytes: Vec<u8>) -> Result<Self, SpawnError> {
        let is_running = Arc::new(AtomicBool::new(true));

        let (stdin_reader, stdin_writer) = pipe()?;
        let (stdout_reader, stdout_writer) = pipe()?;

        //? BufWriter for stdin since writes aren't blocking.
        //? Channel for stdout so we can select whether reads should be
        //? blocking or not.
        let stdin_writer = Arc::new(Mutex::new(BufWriter::new(stdin_writer)));
        let (stdout_tx, stdout_rx) = mpsc::channel::<String>();

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

        let wasi_handle = thread::spawn({
            let is_running = is_running.clone();
            move || match handle_start_thread(store, instance, is_running.clone()) {
                Ok(_) => {
                    is_running.store(false, std::sync::atomic::Ordering::SeqCst);
                }
                Err(e) => {
                    is_running.store(false, std::sync::atomic::Ordering::SeqCst);
                    eprintln!("Thread died: {:?}", e);
                }
            }
        });

        let stdout_handle = thread::spawn({
            let is_running = is_running.clone();
            move || {
                let reader = BufReader::new(stdout_reader);
                for line in reader.lines() {
                    if !is_running.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }

                    match line {
                        Ok(message) => {
                            if stdout_tx.send(message).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }

                is_running.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        });

        return Ok(PluginHost {
            stdin_writer: Some(stdin_writer),
            stdout_receiver: Some(stdout_rx),
            is_running,
            _wasi_handle: wasi_handle,
            _output_handle: stdout_handle,
        });
    }

    pub fn send(&self, message: &str) -> Result<(), SendError> {
        if !self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(SendError::NotRunning);
        }

        let mut writer = self
            .stdin_writer
            .as_ref()
            .unwrap()
            .lock()
            .map_err(|_| SendError::LockError)?;
        writeln!(writer, "{}", message)?;
        writer.flush()?;
        Ok(())
    }

    pub fn try_recv(&self) -> Result<String, RecvError> {
        if !self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(RecvError::NotRunning);
        }

        let msg = self.stdout_receiver.as_ref().unwrap().try_recv()?;
        return Ok(msg);
    }

    pub fn recv(&self) -> Result<String, RecvError> {
        if !self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(RecvError::NotRunning);
        }

        let msg = self.stdout_receiver.as_ref().unwrap().recv()?;
        Ok(msg)
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

        if let Some(rx) = self.stdout_receiver.take() {
            drop(rx);
        }
    }
}

fn handle_start_thread(
    mut store: Store<WasiCtx>,
    instance: wasmi::Instance,
    is_running: Arc<AtomicBool>,
) -> Result<(), SpawnError> {
    let start_func = instance
        .get_func(&store, "_start")
        .ok_or(SpawnError::StartNotFound)?
        .clone();

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
