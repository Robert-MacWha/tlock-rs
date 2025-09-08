use wasmi::{Config, Engine, Module};

#[derive(Clone)]
pub struct CompiledPlugin {
    pub engine: Engine,
    pub module: Module,
}

impl CompiledPlugin {
    pub fn new(wasm_bytes: Vec<u8>) -> Result<Self, wasmi::Error> {
        let mut config = Config::default();
        config.consume_fuel(true);
        // https://github.com/wasmi-labs/wasmi/issues/1647
        config.compilation_mode(wasmi::CompilationMode::Eager);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, wasm_bytes)?;

        Ok(CompiledPlugin { engine, module })
    }
}
