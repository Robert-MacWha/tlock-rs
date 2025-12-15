# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Tlock-rs is a modular-focused wallet framework designed for security, modularity, and portability. It consists of three main components:

- **Frontend**: Cross-platform UIs (Tauri, CLI, server-side) that translate raw data to user interfaces
- **Host**: Interface contracts manager, plugin lifecycle handler, message router with trait registry  
- **Plugins**: Self-contained WASM modules implementing business logic, cryptography, and workflows

The architecture prioritizes plugin-only updates for most functionality while reserving host updates only for new fundamental primitives.

## Programming styles

- Never-nester: Avoid deep nesting.  If code is nested more than 3 levels, consider refactoring to exit early or extract into a new function.
- Functional: Avoid unnecessary state, and keep things functional
- Dependency injection: Use traits to perform dependency injection for simpler testing.
- State Safety: Using rust's rich type system, aim to encode state such that invalid states are unrepresentable. This means creating new, narrower types to represent specific states where needed.

## Workspace Structure

This is a Cargo workspace with the following crates:

- `host/`: Main host application that loads and manages plugins
- `tlock-pdk/`: Plugin Development Kit - provides APIs for plugin authors
- `tlock-hdk/`: Host Development Kit - provides APIs for host developers  
- `plugins/plugin/`: Example plugin template (builds to WASM)
- `runtime/`: Shared runtime utilities for async operations
- `wasmi-async/`: Custom async WASM runtime based on wasmi

## Development Commands

### Building
```bash
# Build entire workspace
cargo build

# Build specific crate
cargo build -p tlock-hdk

# Build plugin for WASM target
cargo build --target wasm32-wasip1 -p rust-plugin-template
```

### Testing
```bash
# Run tests for entire workspace
cargo test

# Run tests for specific crate
cargo test -p tlock-pdk
```

### Code Quality
```bash
# Run clippy linter
cargo clippy

# Format code
cargo fmt

# Check code without building
cargo check
```

### Running the Host
```bash
# Run the host application (from host/ directory)
cd host && cargo run
```

## Key Architecture Concepts

### Plugin Communication
- Plugins run in WASM with wasmer runtime
- Communication via std-pipes for async operations
- JSON-RPC message protocol between host and plugins
- Type-safe API contracts defined in tlock-pdk

### Plugin System
- **Handlers**: Functions plugins implement (one per function)  
- **Hooks**: Functions that trigger alongside handlers (multiple allowed)
- **Requests**: Functions exposed by host to plugins
- Routing strategies: Singleton (one plugin) or Broadcast (all capable plugins)

### Storage Model
Three storage scopes with different security guarantees:
- **HSM**: Hardware security module storage (encrypted, non-portable)
- **Encrypted**: User-authenticated encryption (portable between devices)
- **Plaintext**: Unencrypted storage (accessible without authentication)

### Async Compatibility
- Supports both native async (tokio) and browser environments (wasm-bindgen-futures)
- Uses `#[tokio::main(flavor = "current_thread")]` to simulate browser single-threaded environment
- Runtime abstraction in `runtime/` crate handles platform differences

## API Versioning Strategy

Uses tagged enums with `#[serde(other)]` Unknown variants for backward compatibility:
- Protocol evolution handled gracefully (old plugins get Unknown variants)
- Optional fields with `Option<T>` for non-breaking additions
- JSON stability maintained while allowing Rust code improvements