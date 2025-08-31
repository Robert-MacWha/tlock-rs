use std::future::Future;

/// Spawn a future to run in the background.
/// Starts executing immediately without needing to be awaited.
#[cfg(target_arch = "wasm32")]
pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(future);
}

/// Yield control back to the runtime, allowing other tasks to run.
pub async fn yield_now() {
    #[cfg(target_arch = "wasm32")]
    futures::future::ready(()).await;

    #[cfg(not(target_arch = "wasm32"))]
    tokio::task::yield_now().await;
}
