use extism::{Manifest, Plugin, Wasm};
use tlock_pdk::{Add, Sum};

fn main() {
    // let url =
    //     Wasm::url("https://github.com/extism/plugins/releases/latest/download/count_vowels.wasm");
    // let manifest = Manifest::new([url]);

    let file =
        Wasm::file("../plugins/plugin/target/wasm32-unknown-unknown/debug/rust_pdk_template.wasm");
    let manifest = Manifest::new([file]);
    let mut plugin = Plugin::new(&manifest, [], true).unwrap();

    let res = plugin
        .call::<Add, Sum>("add", Add { left: 1, right: 3 })
        .unwrap();
    println!("result: {:?}", res);
}
