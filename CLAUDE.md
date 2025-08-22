# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Tlock-rs is a modular-focused wallet framework that prioritizes modularity, security, and portability. It uses WebAssembly (WASM) plugins with Wasmer runtime for secure execution isolation. The architecture consists of three main components:

- **Host**: Manages plugin lifecycle, routing, permissions, and provides core services (located in `host/`)
- **Plugin Development Kit (PDK)**: Shared library for building plugins (`tlock-pdk/`)  
- **Plugins**: WASM modules implementing wallet functionality (`plugins/`)

## Build Commands

### Building the entire workspace:
```bash
cargo build --workspace
```

### Building individual components:
```bash
# Host application
cargo build

# Plugin Development Kit
cd tlock-pdk && cargo build

# Build plugin as WASM
cd plugins/plugin && cargo build --target wasm32-wasip1
```

### Running the host:
```bash
cargo run
```

### Testing:
```bash
# Run all tests across workspace
cargo test --workspace

# Check for compilation errors
cargo check --workspace
```

## Architecture Details

### WASM Runtime Integration
The host uses Wasmer 6.1.0-rc.3 with WASIX for running plugins. Communication happens through stdin/stdout pipes between the host and WASM modules.

### Key Components:
- **Host (`host/src/main.rs`)**: Demonstrates WASI pipe communication with a WASM module
- **PDK (`tlock-pdk/src/lib.rs`)**: Currently minimal, uses serde for serialization
- **Example Plugin (`plugins/plugin/src/lib.rs`)**: Basic template for plugin development

### Plugin Development:
- Plugins are compiled to `wasm32-wasip1` target
- Use `tlock-pdk` for shared types and utilities
- Communication via stdin/stdout with the host
- Plugins have `cdylib` crate type for WASM compilation

### Cargo Workspace Structure:
The project uses a Cargo workspace with three members:
- `host/` - Main host application
- `tlock-pdk/` - Plugin Development Kit
- `plugins/plugin/` - Example plugin template

### API Versioning Strategy:
The framework uses tagged enums with `#[serde(other)]` for backward compatibility, allowing graceful handling of unknown variants as the protocol evolves.

### Permission System:
Plugins request permissions for three categories:
- **Handlers**: Functions plugins implement (single plugin per handler)
- **Hooks**: Functions that trigger alongside handlers (multiple hooks allowed)
- **Requests**: Functions exposed by host to plugins

### Storage Scopes:
- **HSM**: Hardware security module storage (encrypted, non-portable)
- **Encrypted**: User-authenticated encryption (portable)  
- **Plaintext**: Unencrypted storage (publicly accessible)

## Development Workflow

1. Make changes to host, PDK, or plugins
2. Build the workspace with `cargo build --workspace`
3. For plugin changes, rebuild WASM target: `cargo build --target wasm32-wasip1`
4. Test the host: `cargo run` (reads `plugin.wasm` if present)
5. Run tests: `cargo test --workspace`

## WASM Targets

The following WASM targets are available:
- `wasm32-unknown-unknown` - Basic WASM
- `wasm32-wasip1` - WASI Preview 1 (used for plugins)
- `wasm32-wasip2` - WASI Preview 2