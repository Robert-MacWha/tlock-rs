use std::sync::{Arc, atomic::AtomicBool};

use thiserror::Error;
use tracing::{error, info, trace};
use wasmi::{Func, Store};

use crate::wasi::wasi::WasiCtx;

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
/// Significantly lower fuel does not significantly lower performance.
/// For this reason, I figure it's fine to keep MAX_FUEL very low and to build in
/// async yielding into the `run_wasm` function so it works better in single-thread
/// environments (like within wasm when building to target the web).  If this later
/// becomes a performance issue we can test it properly.
pub const MAX_FUEL: u64 = 100_000;

/// Spawns a new async task to run the given wasm start function. `is_running`
/// is an atomic signal that can be used to forcefully exit the plugin early.
pub fn spawn_wasm(
    store: Store<WasiCtx>,
    start_func: Func,
    is_running: Arc<AtomicBool>,
    max_fuel: Option<u64>,
) -> impl Future<Output = ()> {
    trace!("Spawning plugin task");
    let is_running = is_running.clone();
    async move {
        if let Err(e) = run_wasm(store, start_func, is_running.clone(), max_fuel).await {
            error!("Plugin error: {:?}", e);
        }
        is_running.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// run_wasm manages the plugin's lifecycle. Because
/// wasmi doesn't support plugin intercepts or async execution, we
/// need some manual way of interrupting the plugin every so often to check if
/// it's been killed and yield to other cooperative tasks. Here I do that by setting
/// a low fuel limit, catching the out-of-fuel condition and resuming the plugin
/// when it's not been killed.
async fn run_wasm(
    mut store: Store<WasiCtx>,
    start_func: Func,
    is_running: Arc<AtomicBool>,
    max_fuel: Option<u64>,
) -> Result<(), RunError> {
    trace!("Plugin task started");
    let max_fuel = max_fuel.unwrap_or(MAX_FUEL);

    //? Starts with zero fuel so we fall into the resumable loop
    store.set_fuel(0).unwrap();
    let mut resumable = start_func.call_resumable(&mut store, &[], &mut [])?;

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

                // Jank Time:
                //
                // Continuation from `wasmi_wasi::poll_oneoff` that sets
                // a resume time in the future if the guest requested,
                // here we actually need to wait until that time before
                // resuming the guest.
                if let Some(resume_time) = store.data().resume_time {
                    let now = runtime::now();
                    if resume_time > now {
                        trace!(
                            "Plugin requested sleep until {:?}, sleeping...",
                            resume_time
                        );
                        let dur = resume_time - now;
                        runtime::sleep(dur).await;
                    }
                    store.data_mut().resume_time = None;
                }

                trace!("Plugin out of fuel, yielding...");
                runtime::yield_now().await;

                match out_of_fuel.resume(&mut store, &mut []) {
                    Ok(next) => {
                        trace!("Plugin resumed after yielding");
                        resumable = next;
                    }
                    Err(e) => {
                        error!("Error resuming plugin: {:?}", e);
                        return Err(RunError::ResumeError(e));
                    }
                }
            }
        }
    }
}
