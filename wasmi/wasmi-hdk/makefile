.PHONY: test bench samply clean

# Build the WASM plugin first
build-wasm:
	cargo build --target wasm32-wasip1 -p test-plugin --release
	wasm-opt ../../target/wasm32-wasip1/release/test-plugin.wasm -O3 -o ../../target/wasm32-wasip1/release/test-plugin-opt.wasm

test: build-wasm
	cargo test

bench: build-wasm
	cargo bench

samply: build-wasm
	cargo build --release --bin profile_prime_sieve
	samply record ../../target/release/profile_prime_sieve

clean:
	cargo clean