use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

use futures::io::{AsyncRead, AsyncWrite};
use futures::task::{Context, Poll, Waker};
use std::pin::Pin;

struct Inner {
    buf: VecDeque<u8>,
    closed: bool,
    waker: Option<Waker>,
}

#[derive(Clone)]
pub struct AsyncPipeReader {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Clone)]
pub struct AsyncPipeWriter {
    inner: Arc<Mutex<Inner>>,
}

pub fn async_pipe() -> (AsyncPipeReader, AsyncPipeWriter) {
    let inner = Arc::new(Mutex::new(Inner {
        buf: VecDeque::new(),
        closed: false,
        waker: None,
    }));
    (
        AsyncPipeReader {
            inner: inner.clone(),
        },
        AsyncPipeWriter { inner },
    )
}

impl AsyncRead for AsyncPipeReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let mut inner = self.inner.lock().unwrap();

        if !inner.buf.is_empty() {
            let available = inner.buf.len().min(buf.len());
            for (i, byte) in inner.buf.drain(..available).enumerate() {
                buf[i] = byte;
            }
            Poll::Ready(Ok(available))
        } else if inner.closed {
            Poll::Ready(Ok(0))
        } else {
            inner.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl AsyncWrite for AsyncPipeWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut inner = self.inner.lock().unwrap();
        inner.buf.extend(buf);
        if let Some(w) = inner.waker.take() {
            w.wake();
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut inner = self.inner.lock().unwrap();
        inner.closed = true;
        if let Some(w) = inner.waker.take() {
            w.wake();
        }
        Poll::Ready(Ok(()))
    }
}

impl Read for AsyncPipeReader {
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

impl Write for AsyncPipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        inner.buf.extend(buf);
        if let Some(w) = inner.waker.take() {
            w.wake();
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
