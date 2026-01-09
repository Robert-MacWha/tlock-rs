# Architecture

Lodgelock takes cues from [HomeAssistant's core architecture](https://developers.home-assistant.io/docs/architecture/core) and [Unix philosophy](https://cscie2x.dce.harvard.edu/hw/ch01s06.html) to create a modular, extensible wallet platform.

## Goals

### 90/10 Implementation Rule

90% of the wallet's functionality should be implemented through plugins. Only 10% of features should require host updates.

**Plugin-only updates**:
- New chains using existing models (EVM variants reusing Ethereum types)
- New signature generation methods (MPC, multi-sig wallets)
- New key management approach
- Complex workflows (bridging, social recovery, DeFi protocols)
- UI enhancements and analysis features

**Host updates required**:
- New domains
- New host functions / services

### Security

Plugins are untrusted code. Users should be able to install plugins from third parties, and should be protected from buggy or malicious plugins.

## Architecture

- The **Host** is the secure, stable 'kernel' that manages plugins, routes requests, and provides core services like storage and networking.
- The **Plugins** are modular implementations of wallet functionality. They implement defined domains (Vault, Provider, Coordinator, Page) and communicate through the host.
- The **Frontend** is the user interface layer that interacts with users and presents data from plugins

### Plugins

Plugins are packages of code that implement domains and provide entities. Plugins are implemented as WASM modules (wasm32-wasip1) that the host loads and manages. During execution, plugins are entirely sandboxed and stateless, only able to communicate externally or store data through host calls.

#### Plugin Lifecycle
```mermaid
sequenceDiagram
    participant user
    participant host
    participant plugin

    user ->> host: Load Plugin Binary
    host ->> plugin: `plugin_init`
    critical Plugin Initialization
        plugin ->> host: `host_register_entity`
        host -->> plugin: 
        plugin ->> host: `host_set_state`
        host -->> plugin: 
    end
    plugin -->> host: `plugin_init`
    host -->> user: Loaded Plugin
```

### Domains

Domains are semantic categories that define what an entity does. Each domain has a fixed set of interfaces all implementations must follow. Domains include:

| Domain      | Purpose                             | Example Methods                                                       |
| ----------- | ----------------------------------- | --------------------------------------------------------------------- |
| Vault       | Custody and transfer of assets      | `GetAssets`, `Withdraw`, `GetDepositAddress`                          |
| Provider    | Blockchain interfacing              | `BlockNumber`, `GetBalance`, `GetBlock`, `Call`, `SendRawTransaction` |
| Coordinator | Safe on-chain transaction execution | `GetSession` `GetAssets` `Propose`                                    |
| Page        | UI Rendering                        | `OnLoad` `OnUpdate`                                                   |

Domains are designed to be as generic as possible while providing useful abstractions. A vault may be a simple private key manager on ethereum or a multisig, a hardware wallet, an MPC signer, a privacy pool account, a dapp's internal custodial ledger, or a CEX with an API. So long as it can hold custody of and transfer assets, it can implement the vault domain.

### Entities

Entities are implementations of domains provided by plugins.  A single entity implements a single domain.  A single plugin may register multiple entities across multiple domains.

```
Plugin: eoa-vault
  Entity: vault:abc123 (Vault domain)
  Entity: page:def456 (Page domain)

Plugin: eoa-coordinator
  Entity: coordinator:ghi789 (Coordinator domain)
  Entity: vault:jkl012 (Vault domain)
  Entity: page:mno345 (Page domain)
```

Entities are designed as black boxes. The host and other plugins do not know about their implementation details. They only care what domain they implement. As such, entities communicate securely through their domain-defined interfaces and different entities may rely on each other to provide complex functionality.

## Host Services

## Performance

Plugins are run in sandboxed WASM runtimes. While this provides strong security guarantees, it also introduces some performance overhead compared to native code execution. Lodgelock is built on [wasmer](https://wasmerio.github.io/wasmer/crates/doc/wasmer/), a fast, cross-platform WASM runtime with [good performance](https://wasmruntime.com/en/benchmarks).

 - For desktop and Android frontends, wasmer can use its native JIT or AOT backends.
 - For browser-based frontends, wasmer uses the browser's built-in WASM runtime like V8 or SpiderMonkey. This provides good performance, often better than native JavaScript execution.
 - For IOS frontends, wasmer uses an interpreted backend due to IOS's JIT restrictions. This results in slower performance, which I'll need to benchmark.


### Plugin Communication

The host runs plugin guest instances in sandboxed wasm32-wasip1 runtimes. Bidirectional communication is facilitated over JSON-RPC over STDIO. This is used because it is a simple, highly compatible protocol that works across languages and is resistant to forward/backward compatibility issues.

For more details on the runtime environment, see the [wasi-plugin-framework](https://github.com/Robert-MacWha/wasmi-plugin-framework/tree/wasmer-shared-memory-test) repo.

### Host Calls

The host exposes various services to plugins through host calls. These include:
    - Persistent Storage
    - Network Fetching
    - Creating Entities
    - Requesting Entities
    - Page Management
    - Calling other Entities

For a full list of host calls, see the [tlock-api docs](../crates/tlock-api/src/lib.rs).


