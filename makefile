.PHONY: plugin

# Build an arbitrary plugin: `PLUGIN=plugin_name make plugin`
plugin:
	cargo build --target wasm32-wasip1 -p $(PLUGIN) --release

fmt:
	cargo fmt

lint:
	cargo clippy --workspace --all-targets -- -D warnings