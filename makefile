.PHONY: plugin plugins

# Build an arbitrary plugin: `PLUGIN=plugin_name make plugin`
plugin:
	cargo build --target wasm32-wasip1 -p $(PLUGIN) --release

# Build all plugins (automatically discovers plugins in plugins/ directory)
plugins:
	@for dir in plugins/*/; do \
		if [ -f "$$dir/Cargo.toml" ]; then \
			plugin_name=$$(basename $$dir); \
			echo "Building plugin: $$plugin_name"; \
			cargo build --target wasm32-wasip1 -p $$plugin_name; \
			wasm-tools demangle \
				target/wasm32-wasip1/debug/$$plugin_name.wasm \
				-o target/wasm32-wasip1/debug/$$plugin_name.wasm; \
		fi \
	done

plugins-release:
	@for dir in plugins/*/; do \
		if [ -f "$$dir/Cargo.toml" ]; then \
			plugin_name=$$(basename $$dir); \
			echo "Building plugin: $$plugin_name"; \
			cargo build --target wasm32-wasip1 -p $$plugin_name --release; \
			wasm-opt -O3 --debuginfo \
				--zero-filled-memory \
				target/wasm32-wasip1/release/$$plugin_name.wasm \
				-o target/wasm32-wasip1/release/$$plugin_name.wasm; \
			wasm-tools demangle \
				target/wasm32-wasip1/release/$$plugin_name.wasm \
				-o target/wasm32-wasip1/release/$$plugin_name.wasm; \
		fi \
	done

fmt:
	cargo fmt

lint:
	cargo clippy --workspace --all-targets -- -D warnings
