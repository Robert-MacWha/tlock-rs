use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use futures::AsyncRead;
use log::info;

struct Inner {
    buf: VecDeque<u8>,
    closed: bool,
    waker: Option<Waker>,
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
        waker: None,
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

impl AsyncRead for NonBlockingPipeReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let mut inner = self.inner.lock().unwrap();

        let mut bytes_read = 0;

        // Read available data
        while bytes_read < buf.len() {
            if let Some(b) = inner.buf.pop_front() {
                buf[bytes_read] = b;
                bytes_read += 1;
            } else {
                break;
            }
        }

        if bytes_read > 0 {
            inner.waker = None; // Clear stale waker
            Poll::Ready(Ok(bytes_read))
        } else if inner.closed {
            // EOF
            Poll::Ready(Ok(0))
        } else {
            // No data available, register waker and return pending
            inner.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl Write for NonBlockingPipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        inner.buf.extend(buf);
        if let Some(waker) = inner.waker.take() {
            waker.wake();
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for NonBlockingPipeWriter {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().unwrap();
        info!("Pipe writer dropped, closing pipe");
        inner.closed = true;
        if let Some(waker) = inner.waker.take() {
            waker.wake();
        }
    }
}
