use std::{
    io::{PipeReader, PipeWriter, pipe},
    sync::{Arc, atomic::AtomicBool},
    thread,
    time::Instant,
};

use thiserror::Error;
use wasmi::{Config, Engine, Func, Linker, Module, Store};
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
    #[error("host trap")]
    HostTrap(wasmi::ResumableCallHostTrap),
}

/// PluginInstance is a single static running instance of a plugin
pub struct PluginInstance {
    is_running: Arc<AtomicBool>,
}

// Notes on interuptability and performance implications (Robert's desktop - ryzen 5 3600).
//
// For a task of running a prime number sieve on 10_000 elements, it:
// - takes 499 ms when MAX_FUEL is 100_000_000 (no refuels needed)
// - takes 528 ms when MAX_FUEL is 1_000_000 (8 refuels needed, each one taking 62 ms)
// - takes ~550 ms when MAX_FUEL is 100_000 (83 refuels needed, each one taking ~6.6 ms)
// - takes ~575 ms when MAX_FUEL is 10_000 (336 refuels needed, each one taking ~600 us)
//
// So it seems like significantly lower fuel does not significantly lower performance - for a 100% CPU bound task that's
// reasonably expensive, interrupting it every 600 us is not terribly impactful. For this reason, I figure it's fine
// to keep MAX_FUEL very low and to build in async yielding into the `run_wasm` function so it works better in
// single-thread environments (like within wasm when building to target the web).  If this later becomes a performance
// issue we can test it properly.
const MAX_FUEL: u64 = 10_000;

impl PluginInstance {
    /// Spawns the wasi plugin in a new thread
    pub fn new(wasm_bytes: Vec<u8>) -> Result<(Self, PipeWriter, PipeReader), SpawnError> {
        let is_running = Arc::new(AtomicBool::new(true));

        // Setup pipes
        let (stdin_reader, stdin_writer) = pipe()?;
        let (stdout_reader, stdout_writer) = pipe()?;

        start_plugin(wasm_bytes, &is_running, stdin_reader, stdout_writer)?;

        Ok((PluginInstance { is_running }, stdin_writer, stdout_reader))
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn kill(&self) {
        self.is_running
            .store(false, std::sync::atomic::Ordering::SeqCst);
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

    // Spawn in a new thread so we don't block the main thread with plugin execution
    let is_running = is_running.clone();
    // tokio::spawn(async move {
    //     match run_wasm(store, start_func, is_running.clone()) {
    //         Ok(_) => {
    //             is_running.store(false, std::sync::atomic::Ordering::SeqCst);
    //         }
    //         Err(e) => {
    //             is_running.store(false, std::sync::atomic::Ordering::SeqCst);
    //             eprintln!("Thread died: {:?}", e);
    //         }
    //     }
    // });

    // println!("Spawning plugin thread...");
    thread::spawn(move || {
        let start_time = Instant::now();
        match run_wasm(store, start_func, is_running.clone()) {
            Ok(_) => {
                is_running.store(false, std::sync::atomic::Ordering::SeqCst);
                println!("Thread exited cleanly");
            }
            Err(e) => {
                is_running.store(false, std::sync::atomic::Ordering::SeqCst);
                println!("Thread died: {:?}", e);
            }
        }

        let elapsed = start_time.elapsed();
        println!("Plugin thread finished after {:?}", elapsed);
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
/// TODO: Add async yielding now using platform-conditional code, so it works with tokio for desktop and wasm_bindgen_futures / gloo for wasm environments
/// to that.
/// https://docs.rs/wasmtime/0.27.0/wasmtime/struct.Store.html#method.interrupt_handle
fn run_wasm(
    mut store: Store<WasiCtx>,
    start_func: Func,
    is_running: Arc<AtomicBool>,
) -> Result<(), SpawnError> {
    store.set_fuel(0).unwrap();
    println!("Starting plugin...");
    let mut resumable = start_func.call_resumable(&mut store, &[], &mut [])?;

    let mut start_time = Instant::now();
    loop {
        match resumable {
            wasmi::ResumableCall::Finished => return Ok(()),
            wasmi::ResumableCall::HostTrap(trap) => {
                println!("Host trap: {:?}", trap);
                return Err(SpawnError::HostTrap(trap));
            }
            wasmi::ResumableCall::OutOfFuel(out_of_fuel) => {
                let elapsed = start_time.elapsed();
                println!(
                    "Out of fuel after {:?}, checking if still running...",
                    elapsed
                );
                start_time = Instant::now();
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
