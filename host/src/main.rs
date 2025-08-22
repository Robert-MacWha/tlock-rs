use std::{
    io::{BufRead, BufReader, Write, pipe},
    thread,
};

use wasmi::{Config, Engine, Extern, Linker, Module, Store};
use wasmi_wasi::{
    WasiCtx, WasiCtxBuilder,
    wasi_common::pipe::{ReadPipe, WritePipe},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wasm_path = "plugin.wasm";
    // Let's declare the Wasm module with the text representation.
    let wasm_bytes = std::fs::read(wasm_path)?;

    let (stdin_reader, mut stdin_writer) = pipe()?;
    let (stdout_reader, stdout_writer) = pipe()?;

    let stdin_pipe = ReadPipe::new(stdin_reader);
    let stdout_pipe = WritePipe::new(stdout_writer);

    let config = Config::default();
    let engine = Engine::new(&config);
    let module = Module::new(&engine, wasm_bytes).unwrap();
    let mut linker = <Linker<WasiCtx>>::new(&engine);
    // add wasi to linker
    let wasi = WasiCtxBuilder::new()
        .stdin(Box::new(stdin_pipe))
        .stdout(Box::new(stdout_pipe))
        .build();
    let mut store = Store::new(&engine, wasi);

    wasmi_wasi::add_to_linker(&mut linker, |ctx| ctx).unwrap();
    let instance = linker.instantiate_and_start(&mut store, &module).unwrap();

    let output_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout_reader);
        for line in reader.lines() {
            match line {
                Ok(message) => println!("WASI output: {}", message),
                Err(e) => eprintln!("Error reading output: {}", e),
            }
        }
    });

    writeln!(stdin_writer, "Hello from host!")?;
    writeln!(stdin_writer, "Second message")?;
    writeln!(stdin_writer, "quit")?;

    drop(stdin_writer);

    let f = instance
        .get_export(&store, "_start")
        .and_then(Extern::into_func)
        .unwrap();
    f.call(&mut store, &[], &mut []).unwrap();

    output_handle.join().unwrap();

    Ok(())
}
