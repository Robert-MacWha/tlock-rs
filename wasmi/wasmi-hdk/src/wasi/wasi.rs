use runtime::web_time::{Instant, SystemTime};
use std::{
    io::{Read, Write},
    time::Duration,
};
use tracing::{error, info, trace, warn};

/// A WASI context that can be attached to a wasmi instance. Attaches
/// a subset of WASI syscalls to the instance, allowing it to
/// get args, env vars, read/write to stdin/stdout/stderr, get time, and
/// get random bytes.
///  
/// Intentionally excludes filesystem (except for stdin/stdout/stderr) and network
/// access for improved security and compatibility.
///
/// https://github.com/WebAssembly/WASI/blob/main/legacy/preview1/docs.md
pub struct WasiCtx {
    args: Vec<String>,
    env: Vec<String>,
    stdin_reader: Option<Box<dyn Read + Send + Sync>>,
    stdout_writer: Option<Box<dyn Write + Send + Sync>>,
    stderr_writer: Option<Box<dyn Write + Send + Sync>>,

    // Time when the host should resume the guest from an out-of-fuel yield.
    // If None, the guest should be started immediately. If Some, the guest
    // should be resumed after this time is reached.
    pub resume_time: Option<Instant>,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum Errno {
    Success = 0,
    Access = 2,
    Again = 6,
    Badf = 8,
    Fault = 21,
    Inval = 28,
    Io = 29,
}

impl Default for WasiCtx {
    fn default() -> Self {
        Self::new()
    }
}

impl WasiCtx {
    pub fn new() -> Self {
        Self {
            args: vec![],
            env: vec![],
            stdin_reader: None,
            stdout_writer: None,
            stderr_writer: None,
            resume_time: None,
        }
    }

    #[allow(dead_code)]
    pub fn add_arg(mut self, arg: &str) -> Self {
        self.args.push(arg.to_string());
        self
    }

    #[allow(dead_code)]
    pub fn add_env(mut self, key: &str, value: &str) -> Self {
        self.env.push(format!("{}={}", key, value));
        self
    }

    pub fn set_stdin<R: Read + Send + Sync + 'static>(mut self, reader: R) -> Self {
        self.stdin_reader = Some(Box::new(reader));
        self
    }

    pub fn set_stdout<W: Write + Send + Sync + 'static>(mut self, writer: W) -> Self {
        self.stdout_writer = Some(Box::new(writer));
        self
    }

    pub fn set_stderr<W: Write + Send + Sync + 'static>(mut self, writer: W) -> Self {
        self.stderr_writer = Some(Box::new(writer));
        self
    }
}

/// Adds the WASI context to the given wasmi linker.
pub fn add_to_linker(linker: &mut wasmi::Linker<WasiCtx>) -> Result<(), wasmi::Error> {
    linker.func_wrap("wasi_snapshot_preview1", "args_get", args_get)?;
    linker.func_wrap("wasi_snapshot_preview1", "args_sizes_get", args_sizes_get)?;
    linker.func_wrap("wasi_snapshot_preview1", "environ_get", env_get)?;
    linker.func_wrap("wasi_snapshot_preview1", "environ_sizes_get", env_sizes_get)?;
    linker.func_wrap("wasi_snapshot_preview1", "fd_read", fd_read)?;
    linker.func_wrap("wasi_snapshot_preview1", "fd_fdstat_get", fd_fdstat_get)?;
    linker.func_wrap("wasi_snapshot_preview1", "fd_write", fd_write)?;
    linker.func_wrap("wasi_snapshot_preview1", "clock_time_get", clock_time_get)?;
    linker.func_wrap("wasi_snapshot_preview1", "fd_close", fd_close)?;
    linker.func_wrap("wasi_snapshot_preview1", "random_get", random_get)?;
    linker.func_wrap("wasi_snapshot_preview1", "sched_yield", sched_yield)?;
    linker.func_wrap("wasi_snapshot_preview1", "poll_oneoff", poll_oneoff)?;
    linker.func_wrap("wasi_snapshot_preview1", "proc_raise", proc_raise)?;
    linker.func_wrap("wasi_snapshot_preview1", "proc_exit", proc_exit)?;

    Ok(())
}

/// Read command-line argument data. The size of the array should match that returned by args_sizes_get. Each argument is expected to be \0 terminated.
fn args_get(mut caller: wasmi::Caller<'_, WasiCtx>, argv: u32, argv_buf: u32) -> i32 {
    trace!("wasi args_get({}, {})", argv, argv_buf);

    let ctx = caller.data_mut();
    let args = &ctx.args.clone();
    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .expect("guest must have memory");

    // Pointer in guest memory to an array of pointers that will be filled with the start of each argument string.
    let mut argv_ptr = argv as usize;
    // Pointer in guest memory to a buffer that will be filled with the `\0`-terminated argument strings.
    let mut buf_ptr = argv_buf as usize;

    for arg in args {
        if write_pointer_and_string(&mut caller, &memory, &mut argv_ptr, &mut buf_ptr, arg).is_err()
        {
            return Errno::Fault as i32;
        }
    }

    Errno::Success as i32
}

/// Returns the number of arguments and the size of the argument string data, or an error.
fn args_sizes_get(mut caller: wasmi::Caller<'_, WasiCtx>, offset0: u32, offset1: u32) -> i32 {
    trace!("wasi args_sizes_get({}, {})", offset0, offset1);

    let ctx = caller.data_mut();
    let argc = ctx.args.len() as u32;
    let argv_buf_len: u32 = ctx.args.iter().map(|s| s.len() as u32 + 1).sum(); // +1 for null terminator

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .expect("guest must have memory");

    if memory
        .write(&mut caller, offset0 as usize, &argc.to_le_bytes())
        .is_err()
    {
        return Errno::Fault as i32;
    }
    if memory
        .write(&mut caller, offset1 as usize, &argv_buf_len.to_le_bytes())
        .is_err()
    {
        return Errno::Fault as i32;
    }

    Errno::Success as i32
}

/// Read environment variable data. The sizes of the buffers should match that
/// returned by environ_sizes_get. Key/value pairs are expected to be joined
/// with =s, and terminated with \0s.
fn env_get(mut caller: wasmi::Caller<'_, WasiCtx>, environ: u32, environ_buf: u32) -> i32 {
    trace!("wasi environ_get({}, {})", environ, environ_buf);

    let ctx = caller.data_mut();
    let env = &ctx.env.clone();
    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .expect("guest must have memory");

    // Pointer in guest memory to an array of pointers that will be filled with the start of each environment string.
    let mut environ_ptr = environ as usize;
    // Pointer in guest memory to a buffer that will be filled with the `\0`-terminated environment strings.
    let mut buf_ptr = environ_buf as usize;

    for var in env {
        if write_pointer_and_string(&mut caller, &memory, &mut environ_ptr, &mut buf_ptr, var)
            .is_err()
        {
            return Errno::Fault as i32;
        }
    }

    Errno::Success as i32
}

/// Returns the number of environment variable arguments and the size of the environment variable data.
fn env_sizes_get(mut caller: wasmi::Caller<'_, WasiCtx>, offset0: u32, offset1: u32) -> i32 {
    trace!("wasi environ_sizes_get({}, {})", offset0, offset1);

    let ctx = caller.data_mut();
    let envc = ctx.env.len() as u32;
    let env_buf_len: u32 = ctx.env.iter().map(|s| s.len() as u32 + 1).sum(); // +1 for null terminator

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .expect("guest must have memory");

    if memory
        .write(&mut caller, offset0 as usize, &envc.to_le_bytes())
        .is_err()
    {
        return Errno::Fault as i32;
    }
    if memory
        .write(&mut caller, offset1 as usize, &env_buf_len.to_le_bytes())
        .is_err()
    {
        return Errno::Fault as i32;
    }

    Errno::Success as i32
}

/// Read from a file descriptor. Note: This is similar to readv in POSIX.
/// Basically that means that instead of reading into a single buffer, we read
/// into multiple buffers described by an array of iovec structures. So we
/// need to read `iov_len` elements from the `iov_ptr` array, each of which
/// describes a buffer we need to fill with data read from this file descriptor.
///
/// - `fd`: The file descriptor.
/// - `iov_ptr`: Pointer to an array of iovec structures.
/// - `iov_len`: Number of iovec structures in the array
/// - `nread_ptr`: Number of bytes read.
///  
/// For now, we only bother implementing reading from fd 0 (stdin).
fn fd_read(
    mut caller: wasmi::Caller<'_, WasiCtx>,
    fd: u32,
    iov_ptr: i32,
    iov_len: i32,
    nread_ptr: i32,
) -> i32 {
    trace!(
        "wasi fd_read({}, {}, {}, {})",
        fd, iov_ptr, iov_len, nread_ptr
    );

    if fd != 0 {
        return Errno::Badf as i32;
    }

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .expect("guest must have memory");

    let mut total_read = 0usize;

    for i in 0..iov_len {
        let base = (iov_ptr as usize) + (i as usize * 8);
        let mut buf_bytes = [0u8; 4];

        memory.read(&caller, base, &mut buf_bytes).unwrap();
        let buf_addr = u32::from_le_bytes(buf_bytes) as usize;

        memory.read(&caller, base + 4, &mut buf_bytes).unwrap();
        let buf_len = u32::from_le_bytes(buf_bytes) as usize;

        // Allocate host buffer
        let mut host_buf = vec![0u8; buf_len];

        // Narrow scope: borrow reader only while calling `read`
        let n = {
            let ctx = caller.data_mut();
            match ctx.stdin_reader.as_mut() {
                Some(r) => match r.read(&mut host_buf) {
                    Ok(n) => n,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        trace!("fd_read: WouldBlock, yielding to host");
                        // TODO: Consider setting an early-return check.
                        // I notice we're polling this *a lot* while waiting for the host to
                        // respond, and I can't imagine that's efficient.
                        let _ = sched_yield(caller);
                        return Errno::Again as i32;
                    }
                    Err(_) => return Errno::Fault as i32,
                },
                None => return Errno::Badf as i32,
            }
        }; // <- borrow of ctx ends here

        if n == 0 {
            trace!("fd_read: EOF");
            break;
        }

        total_read += n;
        memory.write(&mut caller, buf_addr, &host_buf[..n]).unwrap();
        if n < buf_len {
            break; // short read
        }
    }

    memory
        .write(
            &mut caller,
            nread_ptr as usize,
            &(total_read as u32).to_le_bytes(),
        )
        .unwrap();

    trace!("fd_read: total_read={}", total_read);
    Errno::Success as i32
}

/// Write to a file descriptor. Note: This is similar to writev in POSIX.
///
/// # Parameters
///
/// - `fd`: The file descriptor.
/// - `ciov_ptr`: Pointer to an array of iovec structures.
/// - `ciov_len`: Number of iovec structures in the array
/// - `nwrite_ptr`: Number of bytes written.
///  
/// For now we only bother implementing writing to fd 1 (stdout) and fd 2 (stderr).
fn fd_write(
    mut caller: wasmi::Caller<'_, WasiCtx>,
    fd: i32,
    ciov_ptr: i32,
    ciov_len: i32,
    nwrite_ptr: i32,
) -> i32 {
    trace!(
        "wasi fd_write({}, {}, {}, {})",
        fd, ciov_ptr, ciov_len, nwrite_ptr
    );

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .expect("guest must have memory");

    let mut total_written = 0usize;

    for i in 0..ciov_len {
        // Each ciovec is { buf: u32, len: u32 }
        let base = (ciov_ptr as usize) + (i as usize * 8);

        let mut buf_bytes = [0u8; 4];

        // Read buf pointer
        memory.read(&caller, base, &mut buf_bytes).unwrap();
        let buf_addr = u32::from_le_bytes(buf_bytes) as usize;

        // Read buf length
        memory.read(&caller, base + 4, &mut buf_bytes).unwrap();
        let buf_len = u32::from_le_bytes(buf_bytes) as usize;

        // Copy from guest memory
        let mut host_buf = vec![0u8; buf_len];
        if memory.read(&caller, buf_addr, &mut host_buf).is_err() {
            return Errno::Fault as i32;
        }

        // Borrow writer and write message
        //? Narrow scope since we're also mutably borrowing caller elsewhere for memory access
        let ctx = caller.data_mut();
        let writer = match fd {
            1 => ctx.stdout_writer.as_mut(),
            2 => ctx.stderr_writer.as_mut(),
            _ => return Errno::Badf as i32,
        };
        let writer = match writer {
            Some(w) => w,
            None => return Errno::Badf as i32,
        };

        match writer.write(&host_buf) {
            Ok(n) => {
                total_written += n;
                if n < buf_len {
                    break; // partial write, stop
                }
            }
            Err(_) => return Errno::Io as i32,
        }
    }

    // Write total_written into nwrite_ptr
    if memory
        .write(
            &mut caller,
            nwrite_ptr as usize,
            &(total_written as u32).to_le_bytes(),
        )
        .is_err()
    {
        return Errno::Fault as i32;
    }

    Errno::Success as i32
}

fn fd_fdstat_get(mut caller: wasmi::Caller<'_, WasiCtx>, fd: i32, buf_ptr: i32) -> i32 {
    trace!("wasi fd_fdstat_get({}, {})", fd, buf_ptr);

    // Constants from wasi_snapshot_preview1
    const FILETYPE_CHARACTER_DEVICE: u8 = 2;

    // fdflags (none set)
    let fs_flags: u16 = 0;

    // rights (for simplicity, we just allow read or write)
    const RIGHTS_FD_READ: u64 = 1 << 1;
    const RIGHTS_FD_WRITE: u64 = 1 << 6;

    let (filetype, rights_base) = match fd {
        0 => (FILETYPE_CHARACTER_DEVICE, RIGHTS_FD_READ),
        1 | 2 => (FILETYPE_CHARACTER_DEVICE, RIGHTS_FD_WRITE),
        _ => return Errno::Badf as i32,
    };

    let rights_inheriting: u64 = 0;

    let mut buf = [0u8; 24];
    buf[0] = filetype;
    buf[1..3].copy_from_slice(&fs_flags.to_le_bytes());
    buf[8..16].copy_from_slice(&rights_base.to_le_bytes());
    buf[16..24].copy_from_slice(&rights_inheriting.to_le_bytes());

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .expect("guest must have memory");

    if memory.write(&mut caller, buf_ptr as usize, &buf).is_err() {
        return Errno::Fault as i32;
    }

    Errno::Success as i32
}

/// Close a file descriptor. For now we only support closing stdin, stdout, stderr.
fn fd_close(mut caller: wasmi::Caller<'_, WasiCtx>, fd: i32) -> i32 {
    trace!("wasi fd_close({})", fd);

    let ctx = caller.data_mut();

    match fd {
        0 => ctx.stdin_reader = None,
        1 => ctx.stdout_writer = None,
        2 => ctx.stderr_writer = None,
        _ => return Errno::Badf as i32,
    }

    Errno::Success as i32
}

fn clock_time_get(
    mut caller: wasmi::Caller<'_, WasiCtx>,
    clock_id: i32,
    _precision: i64,
    result_ptr: i32,
) -> i32 {
    trace!("wasi clock_time_get({}, _, {})", clock_id, result_ptr);

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .expect("guest must have memory");

    let now = match clock_id {
        // Realtime: nanoseconds since UNIX epoch
        0 | 1 => {
            match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
                Ok(dur) => dur.as_nanos() as u64,
                Err(_) => return Errno::Inval as i32, // time before epoch shouldn't happen
            }
        }
        _ => {
            warn!("unsupported clock_id {}", clock_id);
            return Errno::Inval as i32; // unsupported clock
        }
    };

    if memory
        .write(&mut caller, result_ptr as usize, &now.to_le_bytes())
        .is_err()
    {
        return Errno::Fault as i32;
    }
    Errno::Success as i32
}

fn random_get(mut caller: wasmi::Caller<'_, WasiCtx>, buf_ptr: i32, buf_len: i32) -> i32 {
    trace!("wasi random_get({}, {})", buf_ptr, buf_len);

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .expect("guest must have memory");

    let mut buf = vec![0u8; buf_len as usize];
    if let Err(e) = getrandom::getrandom(&mut buf) {
        eprintln!("random_get failed: {:?}", e);
        return Errno::Io as i32;
    }

    if memory.write(&mut caller, buf_ptr as usize, &buf).is_err() {
        return Errno::Fault as i32;
    }

    Errno::Success as i32
}

fn sched_yield(mut caller: wasmi::Caller<'_, WasiCtx>) -> i32 {
    trace!("wasi sched_yield()");

    // TODO: Would probably better to yield here instead of setting fuel to 0.
    // I can't test this, but I expect interrupting & restarting execution isn't very
    // effective, expecially for repeated calls like from `fd_read` loops.
    caller.set_fuel(0).unwrap();

    Errno::Success as i32
}

const CLOCK_EVENT_TYPE: u8 = 0;

/// Concurrently poll for the occurrence of a set of events.
///
/// Essentially allows the guest to wait for multiple events (timers, fd write/read
/// readiness - subscription_u).
///
/// Based on my understanding & reading other implementations, the function is
/// stateless. So each time it's called it waits until one of the subscriptions
/// is ready, writes the corresponding events into `events_ptr` and returns the
/// number of events written.
///
/// It is not persistent across calls - so if a guest wants to wait for multiple
/// events, it needs to call this function with the same subscriptions again.
///
/// For this impl we only care about timers, so we'll ignore the fd subscriptions
/// and return immediately with no event.
fn poll_oneoff(
    mut caller: wasmi::Caller<'_, WasiCtx>,
    subs_ptr: i32,
    events_ptr: i32,
    nsubscriptions: i32,
    result_ptr: i32,
) -> i32 {
    info!(
        "wasi poll_oneoff({}, {}, {}, {})",
        subs_ptr, events_ptr, nsubscriptions, result_ptr
    );

    if nsubscriptions == 0 {
        error!("poll_oneoff called with zero subscriptions");
        return Errno::Inval as i32;
    }

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .expect("guest must have memory");

    let mut clock_subscriptions = Vec::new();
    for i in 0..nsubscriptions {
        // Each subscription is 48 bytes
        let sub_offset = (subs_ptr as usize) + (i as usize * 48);

        // Read the subscription struct
        // subscription layout (48 bytes total):
        // - userdata: u64 at offset 0
        // - type: u8 at offset 8 (eventtype: 0=clock, 1=fd_read, 2=fd_write)
        // - padding: 7 bytes
        // - union data: 32 bytes at offset 16
        //
        // For clock subscription (subscription_clock at offset 16):
        // - id: u32 (clockid) at offset 16
        // - padding: 4 bytes
        // - timeout: u64 (timestamp) at offset 24
        // - precision: u64 (timestamp) at offset 32
        // - flags: u16 (subclockflags) at offset 40

        let mut sub_bytes = [0u8; 48];
        memory
            .read(&caller, sub_offset as usize, &mut sub_bytes)
            .unwrap();

        let userdata = u64::from_le_bytes(sub_bytes[0..8].try_into().unwrap());
        let sub_type = sub_bytes[8];

        if sub_type != CLOCK_EVENT_TYPE {
            warn!(
                "poll_oneoff: only clock subscriptions supported, got type {}",
                sub_type
            );
            continue;
        }

        let clock_id = u32::from_le_bytes(sub_bytes[16..20].try_into().unwrap());
        let timeout_ns = u64::from_le_bytes(sub_bytes[24..32].try_into().unwrap());
        let precision_ns = u64::from_le_bytes(sub_bytes[32..40].try_into().unwrap());
        let flags = u16::from_le_bytes(sub_bytes[40..42].try_into().unwrap());

        if clock_id > 1 {
            // Only realtime (0) and monotonic (1)
            warn!("poll_oneoff: unsupported clock_id {}", clock_id);
            continue;
        }

        let is_absolute = (flags & 1) != 0;
        trace!(
            "poll_oneoff: clock_id={}, timeout={}, precision={}, flags={:#x}, is_absolute={}",
            clock_id, timeout_ns, precision_ns, flags, is_absolute
        );

        clock_subscriptions.push((userdata, clock_id, timeout_ns, is_absolute));
    }

    info!(
        "poll_oneoff: {} clock subscriptions",
        clock_subscriptions.len()
    );

    if clock_subscriptions.is_empty() {
        warn!("poll_oneoff: no valid clock subscriptions found");
        return Errno::Inval as i32;
    }

    // Convert all relative timeouts to absolute deadlines
    let now_ns = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(dur) => dur.as_nanos() as u64,
        Err(_) => return Errno::Inval as i32, // time before epoch shouldn't happen
    };

    clock_subscriptions.iter_mut().for_each(|sub| {
        if !sub.3 {
            sub.2 = now_ns.saturating_add(sub.2);
        }
    });

    // Find the earliest deadline
    let (earliest_userdata, _earliest_clock_id, earliest_deadline_ns, _) =
        clock_subscriptions.iter().min_by_key(|sub| sub.2).unwrap();

    // Jank Time:
    //
    // Basically, poll_oneoff MUST block from the guest's perspective until 1+
    // events are ready. If we try to return immediately with no events (either)
    // via an Errno::Again or Errno::Success with 0 events, the guest will panic.
    //
    // We can't block here since that this isn't an async block (due to wasmi's API)
    // and we don't want to block the entire executor thread since we're in a single-
    // threaded-context. SO instead we instantly return a successful result for the
    // earliest timer, and if the deadline hasn't been reached yet we set the fuel
    // to zero to yield execution back to the host. The host can then notice that
    // `resume_time` is Some(time in the future) and will that time before
    // resuming execution of the guest.
    //
    // TODO: Fix this mess

    //? Future deadline trap
    if earliest_deadline_ns > &now_ns {
        let wait_duration = Duration::from_nanos(earliest_deadline_ns - now_ns);
        caller.data_mut().resume_time = Some(Instant::now() + wait_duration);
        info!(
            "poll_oneoff: waiting for {:?} before resuming guest",
            wait_duration
        );

        //? Need to yield, so set fuel to zero
        caller.set_fuel(0).unwrap();
    }

    // Write the event for the earliest deadline
    // size: 32
    // - userdata: at offset 0
    // - error: at offset 8
    // - type: at offset 10
    // - fd_readwrite: at offset 16 (empty for clock events)
    let mut event_buf = [0u8; 32];
    event_buf[0..8].copy_from_slice(&earliest_userdata.to_le_bytes());
    event_buf[8..10].copy_from_slice(&(Errno::Success as u16).to_le_bytes());
    event_buf[10] = CLOCK_EVENT_TYPE;
    // fd_readwrite is empty for clock events

    info!(
        "Writing event: userdata={}, error={}, type={}",
        earliest_userdata,
        Errno::Success as u16,
        CLOCK_EVENT_TYPE
    );
    info!("Event buffer: {:?}", &event_buf[..16]);

    let event_offset = events_ptr as usize;
    memory.write(&mut caller, event_offset, &event_buf).unwrap();

    // Write number of events (1) into result_ptr
    memory
        .write(&mut caller, result_ptr as usize, &1u32.to_le_bytes())
        .unwrap();

    Errno::Success as i32
}

fn proc_raise(_caller: wasmi::Caller<'_, WasiCtx>, sig: i32) -> Result<(), wasmi::Error> {
    info!("wasi proc_raise({})", sig);

    // https://github.com/WebAssembly/WASI/blob/main/legacy/preview1/docs.md#signal
    match sig {
        0 | 13 | 16 | 17 | 22 | 27 => {
            // Ignored signals
            Ok(())
        }
        _ => {
            // Termination signals
            Err(wasmi::Error::i32_exit(128 + sig))
        }
    }
}

fn proc_exit(_caller: wasmi::Caller<'_, WasiCtx>, status: i32) -> Result<(), wasmi::Error> {
    info!("wasi proc_exit({})", status);
    Err(wasmi::Error::i32_exit(status))
}

/// Write a pointer to a string into a table, and the string itself into a buffer,
/// then advance the table and buffer pointers.
///
/// Utility for writing into guest memory.
fn write_pointer_and_string(
    caller: &mut wasmi::Caller<'_, WasiCtx>,
    memory: &wasmi::Memory,
    table_ptr: &mut usize,
    buf_ptr: &mut usize,
    s: &str,
) -> Result<(), Errno> {
    // 1. Write pointer into table
    let ptr_le = (*buf_ptr as u32).to_le_bytes();
    memory
        .write(&mut *caller, *table_ptr, &ptr_le)
        .map_err(|_| Errno::Fault)?;
    *table_ptr += 4;

    // 2. Write string+null into buffer
    let mut tmp = Vec::with_capacity(s.len() + 1);
    tmp.extend_from_slice(s.as_bytes());
    tmp.push(0); // null terminator
    memory
        .write(&mut *caller, *buf_ptr, &tmp)
        .map_err(|_| Errno::Fault)?;
    *buf_ptr += tmp.len();

    Ok(())
}
