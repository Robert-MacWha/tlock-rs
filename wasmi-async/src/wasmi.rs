use std::sync::{Arc, atomic::AtomicBool};

use runtime::{spawn_local, yield_now};
use thiserror::Error;
use wasmi::{Func, Store};
use wasmi_wasi::WasiCtx;

#[derive(Error, Debug)]
pub enum RunError {
    #[error("wasmi error")]
    WasmiError(#[from] wasmi::Error),
    #[error("resume error")]
    ResumeError(wasmi::Error),
    #[error("host trap")]
    HostTrap(wasmi::ResumableCallHostTrap),
}

/// Notes on interuptability and performance implications (Robert's desktop - ryzen 5 3600).
///
/// For a task of running a prime number sieve on 10_000 elements, it:
/// - takes 499 ms when MAX_FUEL is 100_000_000 (no refuels needed)
/// - takes 528 ms when MAX_FUEL is 1_000_000 (8 refuels needed, each one taking 62 ms)
/// - takes ~550 ms when MAX_FUEL is 100_000 (83 refuels needed, each one taking ~6.6 ms)
/// - takes ~575 ms when MAX_FUEL is 10_000 (836 refuels needed, each one taking ~600 us)
///
/// So it seems like significantly lower fuel does not significantly lower performance.
/// For this reason, I figure it's fine to keep MAX_FUEL very low and to build in
/// async yielding into the `run_wasm` function so it works better in single-thread
/// environments (like within wasm when building to target the web).  If this later
/// becomes a performance issue we can test it properly.
const MAX_FUEL: u64 = 10_000;

pub fn spawn_wasm(
    store: Store<WasiCtx>,
    start_func: Func,
    is_running: Arc<AtomicBool>,
    max_fuel: Option<u64>,
) -> Arc<AtomicBool> {
    println!("Spawning plugin");
    let _ = spawn_local({
        let is_running = is_running.clone();
        async move {
            if let Err(e) = run_wasm(store, start_func, is_running.clone(), max_fuel).await {
                eprintln!("Plugin error: {:?}", e);
            }
            is_running.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    });

    is_running
}

/// run_wasm manages the plugin's lifecycle. Essentially - because
/// wasmi doesn't support any plugin intercept, halting, or async execution, we
/// need some manual way of interrupting the plugin every so often to check if
/// it's been killed and yield. Here I do that by setting a low fuel limit,
/// catching the out-of-fuel condition and resuming the plugin when it's not killed.
pub async fn run_wasm(
    mut store: Store<WasiCtx>,
    start_func: Func,
    is_running: Arc<AtomicBool>,
    max_fuel: Option<u64>,
) -> Result<(), RunError> {
    let max_fuel = max_fuel.unwrap_or(MAX_FUEL);

    //? Starts with zero fuel so we fall into the resumable loop that yields
    store.set_fuel(0).unwrap();
    let mut resumable = start_func.call_resumable(&mut store, &[], &mut [])?;

    println!("Plugin started");

    loop {
        match resumable {
            wasmi::ResumableCall::Finished => return Ok(()),
            wasmi::ResumableCall::HostTrap(trap) => {
                return Err(RunError::HostTrap(trap));
            }
            wasmi::ResumableCall::OutOfFuel(out_of_fuel) => {
                if !is_running.load(std::sync::atomic::Ordering::SeqCst) {
                    return Ok(());
                }

                let required = out_of_fuel.required_fuel();
                let top_up = required.max(max_fuel);
                store.set_fuel(top_up).unwrap();

                yield_now().await;

                match out_of_fuel.resume(&mut store, &mut []) {
                    Ok(next) => resumable = next,
                    Err(e) => return Err(RunError::ResumeError(e)),
                }
            }
        }
    }
}
