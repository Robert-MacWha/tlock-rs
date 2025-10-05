use std::{
    io::{Read, Write},
    sync::{Arc, atomic::AtomicBool},
};

use crate::compiled_plugin::CompiledPlugin;
use thiserror::Error;
use wasmi::{Linker, Store};
use wasmi_async::{
    non_blocking_pipe::{NonBlockingPipeReader, NonBlockingPipeWriter, non_blocking_pipe},
    wasmi::spawn_wasm,
    wasmi_wasi::{WasiCtx, add_to_linker},
};

#[derive(Error, Debug)]
pub enum SpawnError {
    #[error("io error")]
    IoError(#[from] std::io::Error),
    #[error("start not found")]
    StartNotFound,
    #[error("wasmi error")]
    WasmiError(#[from] wasmi::Error),
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
            impl Future<Output = ()>,
        ),
        SpawnError,
    > {
        let is_running = Arc::new(AtomicBool::new(true));

        // Setup pipes
        let (stdin_reader, stdin_writer) = non_blocking_pipe();
        let (stdout_reader, stdout_writer) = non_blocking_pipe();
        let (stderr_reader, stderr_writer) = non_blocking_pipe();

        let fut = start_plugin(
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
            fut,
        ))
    }

    #[allow(unused)]
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
) -> Result<impl Future<Output = ()>, SpawnError>
where
    R: Read + Send + Sync + 'static,
    W1: Write + Send + Sync + 'static,
    W2: Write + Send + Sync + 'static,
{
    let module = compiled.module;
    let engine = compiled.engine;

    let mut linker = Linker::new(&engine);
    let wasi = WasiCtx::new()
        .set_stdin(Box::new(stdin_reader))
        .set_stdout(Box::new(stdout_writer))
        .set_stderr(Box::new(stderr_writer));

    let mut store = Store::new(&engine, wasi);
    add_to_linker(&mut linker)?;

    let instance = linker.instantiate_and_start(&mut store, &module)?;
    let start_func = instance
        .get_func(&store, "_start")
        .ok_or(SpawnError::StartNotFound)?;

    let fut = spawn_wasm(store, start_func, is_running.clone(), None);

    Ok(fut)
}
