use extism_pdk::*;
use tlock_pdk::{Add, Sum};

// start with something simple
#[plugin_fn]
pub fn greet(name: String) -> FnResult<String> {
    Ok(format!("Hello, {}!", name))
}

#[plugin_fn]
pub fn add(input: Add) -> FnResult<Sum> {
    Ok(Sum {
        value: input.left + input.right,
    })
}
