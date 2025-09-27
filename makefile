.PHONY: test bench samply clean

# Build the WASM plugin first
build-wasm:
	cargo build --target wasm32-wasip1 -p test-plugin --release

test: build-wasm
	cargo test

bench: build-wasm
	cargo bench

samply: build-wasm
	cargo build --release --bin profile_prime_sieve
	samply record target/release/profile_prime_sieve

# Build an arbitrary plugin: `make plugin PLUGIN=plugin_name`
plugin:
	cargo build --target wasm32-wasip1 -p $(PLUGIN) --release

clean:
	cargo clean