#[cfg(target_arch = "wasm32")]
pub async fn yield_now() {
    use gloo_timers::future::TimeoutFuture;
    TimeoutFuture::new(0).await;
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn yield_now() {
    tokio::task::yield_now().await;
}

#[cfg(target_arch = "wasm32")]
pub fn spawn_local<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_local<F>(future: F) -> tokio::task::JoinHandle<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    tokio::task::spawn(future)
}
