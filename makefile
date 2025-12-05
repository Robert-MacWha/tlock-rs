.PHONY: test bench profile clean

# Build the WASM plugin first
build-wasm:
	cargo build --target wasm32-wasip1 -p test-plugin --release

test: build-wasm
	cargo test

bench: build-wasm
	cargo bench

profile: build-wasm
	samply record cargo bench --bench benchmark -- prime_sieve_large --profile-time=20

profile-all: build-wasm
	samply record cargo bench --bench benchmark

# Build an arbitrary plugin: `PLUGIN=plugin_name make plugin`
plugin:
	cargo build --target wasm32-wasip1 -p $(PLUGIN) --release

clean:
	cargo clean