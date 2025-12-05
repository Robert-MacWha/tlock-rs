use std::time::Duration;
pub use web_time;

pub fn yield_now() -> impl std::future::Future<Output = ()> {
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    {
        use gloo_timers::future::TimeoutFuture;
        TimeoutFuture::new(0)
    }

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    {
        tokio::task::yield_now()
    }
}

pub fn now() -> web_time::Instant {
    web_time::Instant::now()
}

pub async fn sleep(dur: Duration) {
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    {
        gloo_timers::future::TimeoutFuture::new(dur.as_millis() as u32).await;
    }

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    {
        tokio::time::sleep(dur).await;
    }
}
