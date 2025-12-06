.PHONY: plugin

# Build an arbitrary plugin: `PLUGIN=plugin_name make plugin`
plugin:
	cargo build --target wasm32-wasip1 -p $(PLUGIN) --release
