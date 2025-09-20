# Wasmi Async

Wasmi async was designed to let me run wasmi_wasi programs in environments where:
1. The host is limited to single-threaded execution.
2. The host is itself running in a wasm (IE wasm32-unknown-unknown) runtime.

These requirements mean that the runtime is single-threaded, forcefully cooperative, and implements a minimal subset of wasmi-v1 sufficient for most of my needs.