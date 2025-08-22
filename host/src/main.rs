use tlock_pdk::PluginHost;

use std::{
    io::{BufRead, BufReader, Write},
    thread::sleep,
    time::Duration,
};
use wasmer::{Engine, Module, Store};
use wasmer_wasix::{Pipe, WasiEnv, runners::wasi::WasiRunner};

// fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let wasm_bytes = std::fs::read("plugin.wasm")?;
//     let engine = Engine::default();
//     let module = Module::new(&engine, &wasm_bytes)?;

//     let (stdin_tx, stdin_rx) = Pipe::channel();
//     let (stdout_tx, stdout_rx) = Pipe::channel();

//     let mut runner = WasiRunner::new();
//     runner
//         .with_stdin(Box::new(stdin_rx))
//         .with_stdout(Box::new(stdout_tx));

//     std::thread::spawn(move || {
//         runner.run_wasm(
//             wasmer_wasix::runners::wasi::RuntimeOrEngine::Engine(engine),
//             "plugin",
//             module,
//             wasmer_types::ModuleHash::xxhash(wasm_bytes),
//         )
//     });

//     std::thread::spawn(move || {
//         let reader = BufReader::new(stdout_rx);
//         for line in reader.lines() {
//             if let Ok(l) = line {
//                 println!("Plugin stdout: {}", l);
//             }
//         }
//     });

//     let mut stdin = stdin_tx;

//     for _ in 1..100 {
//         println!("Sending: Hello from host!");
//         writeln!(stdin, "Hello from host!")?;
//         stdin.flush()?;
//         std::thread::sleep(std::time::Duration::from_millis(500));
//     }

//     Ok(())
// }

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wasm_path = "plugin.wasm";
    let wasm_bytes = std::fs::read(wasm_path)?;

    println!("Spawning plugin from {}", wasm_path);
    let plugin = PluginHost::spawn("plugin", wasm_bytes)?;

    println!("Sending message to plugin");
    plugin.host_send("Hello from host!".to_string())?;

    let mut recv_count = 0;
    loop {
        match plugin.host_try_recv()? {
            Some(msg) => {
                println!("Received from plugin: {}", msg);
                recv_count += 1;
                let msg = format!("Message {} from host", recv_count);
                plugin.host_send(msg)?;
            }
            None => {
                println!("No message from plugin, waiting...");
                sleep(Duration::from_millis(100));
            }
        }
    }
}
