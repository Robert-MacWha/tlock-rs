use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

struct Inner {
    buf: VecDeque<u8>,
    closed: bool,
}

#[derive(Clone)]
pub struct NonBlockingPipeReader {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Clone)]
pub struct NonBlockingPipeWriter {
    inner: Arc<Mutex<Inner>>,
}

pub fn non_blocking_pipe() -> (NonBlockingPipeReader, NonBlockingPipeWriter) {
    let inner = Arc::new(Mutex::new(Inner {
        buf: VecDeque::new(),
        closed: false,
    }));
    (
        NonBlockingPipeReader {
            inner: inner.clone(),
        },
        NonBlockingPipeWriter { inner },
    )
}

impl Read for NonBlockingPipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        let mut n = 0;
        while n < buf.len() {
            if let Some(b) = inner.buf.pop_front() {
                buf[n] = b;
                n += 1;
            } else {
                break;
            }
        }
        if n == 0 && inner.closed {
            Ok(0)
        } else if n == 0 {
            // no data, behave like a nonblocking pipe
            Err(io::ErrorKind::WouldBlock.into())
        } else {
            Ok(n)
        }
    }
}

impl Write for NonBlockingPipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        inner.buf.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
