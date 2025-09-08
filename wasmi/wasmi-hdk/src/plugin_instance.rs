use std::{
    io::{Read, Write},
    sync::{Arc, atomic::AtomicBool},
};

use crate::{
    compiled_plugin::CompiledPlugin,
    non_blocking_pipe::{NonBlockingPipeReader, NonBlockingPipeWriter, non_blocking_pipe},
};
use thiserror::Error;
use wasmi::{Linker, Store};
use wasmi_async::wasmi::spawn_wasm;
use wasmi_wasi::{
    WasiCtx, WasiCtxBuilder,
    wasi_common::pipe::{ReadPipe, WritePipe},
};

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
}

/// PluginInstance is a single static running instance of a plugin
pub struct PluginInstance {
    is_running: Arc<AtomicBool>,
}

impl PluginInstance {
    /// Spawns the wasi plugin in a new thread
    pub fn new(
        compiled: CompiledPlugin,
    ) -> Result<
        (
            Self,
            NonBlockingPipeWriter,
            NonBlockingPipeReader,
            NonBlockingPipeReader,
        ),
        SpawnError,
    > {
        let is_running = Arc::new(AtomicBool::new(true));

        // Setup pipes
        let (stdin_reader, stdin_writer) = non_blocking_pipe();
        let (stdout_reader, stdout_writer) = non_blocking_pipe();
        let (stderr_reader, stderr_writer) = non_blocking_pipe();

        start_plugin(
            compiled,
            is_running.clone(),
            stdin_reader,
            stdout_writer,
            stderr_writer,
        )?;

        Ok((
            PluginInstance { is_running },
            stdin_writer,
            stdout_reader,
            stderr_reader,
        ))
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn kill(&self) {
        self.is_running
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

fn start_plugin<R, W1, W2>(
    compiled: CompiledPlugin,
    is_running: Arc<AtomicBool>,
    stdin_reader: R,
    stdout_writer: W1,
    stderr_writer: W2,
) -> Result<(), SpawnError>
where
    R: Read + Send + Sync + 'static,
    W1: Write + Send + Sync + 'static,
    W2: Write + Send + Sync + 'static,
{
    let stdin_pipe = ReadPipe::new(stdin_reader);
    let stdout_pipe = WritePipe::new(stdout_writer);
    let stderr_pipe = WritePipe::new(stderr_writer);

    let module = compiled.module;
    let engine = compiled.engine;

    let mut linker = <Linker<WasiCtx>>::new(&engine);
    let wasi = WasiCtxBuilder::new()
        .stdin(Box::new(stdin_pipe))
        .stdout(Box::new(stdout_pipe))
        .stderr(Box::new(stderr_pipe))
        .build();

    let mut store = Store::new(&engine, wasi);
    wasmi_wasi::add_to_linker(&mut linker, |ctx| ctx)?;

    let instance = linker.instantiate_and_start(&mut store, &module)?;
    let start_func = instance
        .get_func(&store, "_start")
        .ok_or(SpawnError::StartNotFound)?;

    spawn_wasm(store, start_func, is_running, None);

    Ok(())
}
