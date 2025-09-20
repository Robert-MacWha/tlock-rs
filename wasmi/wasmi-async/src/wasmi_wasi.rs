use rand::TryRngCore;
use std::io::{Read, Write};

use log::{info, trace};

pub struct WasiCtx {
    args: Vec<String>,
    env: Vec<String>,
    stdin_reader: Option<Box<dyn Read + Send + Sync>>,
    stdout_writer: Option<Box<dyn Write + Send + Sync>>,
    stderr_writer: Option<Box<dyn Write + Send + Sync>>,
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

impl WasiCtx {
    pub fn new() -> Self {
        Self {
            args: vec![],
            env: vec![],
            stdin_reader: None,
            stdout_writer: None,
            stderr_writer: None,
        }
    }

    pub fn add_arg(mut self, arg: &str) -> Self {
        self.args.push(arg.to_string());
        self
    }

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
    info!("Adding WASI functions to linker...");

    linker.func_wrap("wasi_snapshot_preview1", " args_get", args_get)?;
    linker.func_wrap("wasi_snapshot_preview1", " args_sizes_get", args_sizes_get)?;
    linker.func_wrap("wasi_snapshot_preview1", "environ_get", env_get)?;
    linker.func_wrap("wasi_snapshot_preview1", "environ_sizes_get", env_sizes_get)?;
    linker.func_wrap("wasi_snapshot_preview1", "fd_read", fd_read)?;
    linker.func_wrap("wasi_snapshot_preview1", "fd_fdstat_get", fd_fdstat_get)?;
    linker.func_wrap("wasi_snapshot_preview1", "fd_write", fd_write)?;
    linker.func_wrap("wasi_snapshot_preview1", "clock_time_get", clock_time_get)?;
    linker.func_wrap("wasi_snapshot_preview1", "fd_close", fd_close)?;
    linker.func_wrap("wasi_snapshot_preview1", "random_get", random_get)?;
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
                        trace!("fd_read: WouldBlock");
                        caller.set_fuel(0).unwrap(); // Stops execution, yielding to the host and allowing other tasks to run.
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

// TODO: Implement for wasm32-unknown-unknown target
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
        0 => {
            match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                Ok(dur) => dur.as_nanos() as u64,
                Err(_) => return Errno::Inval as i32, // time before epoch shouldn't happen
            }
        }
        // Monotonic: nanoseconds since arbitrary fixed point
        1 => {
            let dur = std::time::Instant::now().elapsed();
            dur.as_nanos() as u64
        }
        _ => return Errno::Inval as i32, // unsupported clock
    };

    if memory
        .write(&mut caller, result_ptr as usize, &now.to_le_bytes())
        .is_err()
    {
        return Errno::Fault as i32;
    }
    Errno::Success as i32
}

// TODO: Implement for wasm32-unknown-unknown target

fn random_get(mut caller: wasmi::Caller<'_, WasiCtx>, buf_ptr: i32, buf_len: i32) -> i32 {
    trace!("wasi random_get({}, {})", buf_ptr, buf_len);

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .expect("guest must have memory");

    let mut buf = vec![0u8; buf_len as usize];
    if let Err(e) = rand::rngs::OsRng.try_fill_bytes(&mut buf) {
        eprintln!("random_get failed: {:?}", e);
        return Errno::Io as i32;
    }

    if memory.write(&mut caller, buf_ptr as usize, &buf).is_err() {
        return Errno::Fault as i32;
    }

    Errno::Success as i32
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
