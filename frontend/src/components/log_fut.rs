use dioxus_logger::tracing::info;
use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

#[pin_project]
pub struct LogFuture<F> {
    name: &'static str,
    #[pin]
    inner: F,
}

impl<F> Future for LogFuture<F>
where
    F: Future,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Safely project to the pinned field
        let mut this = self.project();
        info!("POLLING future: {}", this.name);

        let res = this.inner.as_mut().poll(cx);

        match &res {
            Poll::Pending => info!("→ {} yielded (Pending)", this.name),
            Poll::Ready(_) => info!("→ {} completed (Ready)", this.name),
        }

        res
    }
}

pub fn log_future<F>(name: &'static str, fut: F) -> LogFuture<F>
where
    F: Future,
{
    LogFuture { name, inner: fut }
}
