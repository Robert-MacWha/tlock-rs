# Plugin Development

Plugins for lodgelock are WebAssembly (WASM) modules running in a sandboxed wasm32-wasip1 environment. WASI calls for args, environment variables, clocks, random numbers, yielding, and exits are fully supported. poll_oneoff is partially supported, only for clock timers and stdin readiness. fd_read and fd_write are only supported for stdin, stdout, and stderr. All other WASI calls will trap.

Simply put, these restrictions mean that plugins cannot access the network or filesystem, but are otherwise free to run arbitrary code in a single-threaded environment. Unless you're actively trying to use unsupported features you likely won't run into any issues.

## Plugin Development Kit (PDK)

Lodgelock provides a rust PDK to simplify plugin development. The PDK handles host communication, serialization / deserialization, request routing, and abstractions for common tasks. See the plugins in [`/plugins`](../plugins/) for examples.

(Full guide coming soon)
