
# Lodgelock

Lodgelock is designed as a modular-first wallet framework. It aims to empower users, developers, and the broader web3 ecosystem by providing a secure, extensible, and user-centric wallet platform.

Lodgelock is designed around three core ideals:

1. **Wallets for self-sovereignty**: Wallets are tools for users to manage their money, identity, and data. They should empower users to hold ownership over their digital lives, providing full control and autonomy.
2. **Wallets for security**: Wallets act as guardians of users' assets and information. They are a critical single point of failure and must prioritize security, ensuring that funds, privacy, and integrity are maintained at all times.
3. **Wallets for modularity**: Wallets should act as the kernel for web3 applications. They should provide a secure and unopinionated platform that manages security and resources, leaving the 'user-space' features entirely implemented by modular plugins to the user's discretion.

## Status Quo

The current wallet landscape is dominated by monolithic walled gardens that bundle a fixed set of features and applications. Defi's origin as websites has been a saving grace, allowing users to access a broader ecosystem of applications. However, wallets have not embraced this modularity, instead opting for closed systems that limit user choice and stifle innovation.

This lack of modularity has created a dangerous example of the **Power of Defaults**. Wallets decree what features and applications are present within their walls, shaping behavior and experience for millions. For a prime example, simply look at the exchange rates provided by Metamask's built-in swap versus uniswap or 1inch. When the gateway to web3 is a profit-seeking entity, the default becomes a toll-bridge rather than an open road.

Wallets also suffer from **Extractive Incentives**. When the wallet controls the default experience, it is incentivised to prioritize features that enrich itself over those that benefit users. Consider Polymarket integrating kalshi prediction markets into their app, adding gambling features to the homepage of a financial application without user consent, warning, age verification, or an option to disable it. While users should undoubtedly have the freedom to have such features, they should not be forced upon them.

## Architecture Overview

The primary architectural goals were security and a 90/10 implementation rule. It is architected with three core components: the Host, Plugins, and Frontend.

- The **Host** is the secure, stable 'kernel' that manages plugins, routes requests, and provides core services like storage and networking.
- **Plugins** are modulare implementations of wallet functionality. They implement defined domains (Vault, Provider, Coordinator, Page) and communicate through the host.
- The **Frontend** is the user interface layer that interacts with users and presents data from plugins

The architecture is designed to implement 90% of the wallet's functionality through plugins. This rule means that:

**Plugin-only updates**:
- New chains using existing models (EVM variants reusing Ethereum types)
- New signature generation methods (MPC, multi-sig wallets)
- New key management approach
- Complex workflows (bridging, social recovery, DeFi protocols)
- UI enhancements and analysis features

**Host updates required**:
- New domains
- New host functions / services

## Core Concepts

### Domains

Domains are semantic categories that define what an entity does. Each domain has a fixed set of interfaces all implementations must follow. Domains include:

| Domain      | Purpose                             | Example Methods                                                       |
| ----------- | ----------------------------------- | --------------------------------------------------------------------- |
| Vault       | Custody and transfer of assets      | `GetAssets`, `Withdraw`, `GetDepositAddress`                          |
| Provider    | Blockchain interfacing              | `BlockNumber`, `GetBalance`, `GetBlock`, `Call`, `SendRawTransaction` |
| Coordinator | Safe on-chain transaction execution | `GetSession` `GetAssets` `Propose`                                    |
| Page        | UI Rendering                        | `OnLoad` `OnUpdate`                                                   |

Domains are designed to be as generic as possible while providing useful abstractions. A vault may be a simple private key manager on ethereum or a multisig, a hardware wallet, an MPC signer, a privacy pool account, a dapp's internal custodial ledger, or a CEX with an API. So long as it can custody and transfer assets, it can implement the vault domain.

### Entities

Entities are implementations of domains. Each entity implements a single domain and is managed by a plugin which provides its code and metadata. A single plugin may register multiple entities across multiple domains.

```
Plugin: eoa-vault
  Entity: vault:abc123 (Vault domain)
  Entity: page:def456 (Page domain)
Plugin: eoa-coordinator
  Entity: coordinator:ghi789 (Coordinator domain)
  Entity: vault:jkl012 (Vault domain)
  Entity: page:mno345 (Page domain)
```

Entities are designed as black boxes. The host and other plugins do not know nor care about their internal implementations. They only care what domain they implement. As such, entities communicate securely through their domain-defined interfaces and different entities may rely on each other to provide complex functionality.

### Plugins

Plugins are packages of code that provide entities. Plugins are implemented as WASM modules that the host loads and manages. During execution, plugins are entirely sandboxed and stateless, only able to communicate externally or store data through host calls. Like regular Dapps, 

Plugins are untrusted code - the host must assume they are malicious and enforce strict security boundaries. However, unlike regular Dapps, plugins are installed once by a user after which they are static, locally stored, and can be run entirely offline (assuming the plugin does not require network access).


#### Plugin Lifecycle
```
1. Host loads WASM binary
2. Host calls plugin's `initialize` function (first load only)
3. Plugin registers entities via host::RegisterEntity
4. Host routes incoming requests to registered handlers
5. Plugin makes host calls as needed (storage, network, other entities)
6. Plugin returns response
7. WASM instance terminates (stateless between requests)
```

### 

 - Vision
 - Problem
 - Tlock core features
   - Plugin-only evolution
   - Hard sandboxing
   - Domain system
 -  

---

# Tlock-rs

Tlock is designed as a modular-focused wallet framework.  It is designed to, as much as possible, get out of the way while providing a framework that allows plugins to securely and effectively perform tasks.  Its priorities are:

1. Modularity. Unless there's a very good reason, modular plugins should be responsible for all functionality.

2. Security. Modularity cannot mean a decrease in security for the user's machine, privacy, or money.

3. Portability. Tlock should be usable across arbitrary platforms, with arbitrary public interfaces.

4. Extensibility. New chains, new applications, and new workflows should be easy to add without requiring host updates.

This document contains a system overview.
- See [status-quo.md](./docs/status-quo.md) for problems I have with the current status quo.
- See [design-considerations.md](./docs/design-considerations.md) for problems I have this this proposal.
- See the remainder of the docs/ folder for additional design documents

## Name

Ideas:
- Lodgelock.  Beavers, who are builders, lodge meaning a cabin or inn (shelter) and also to lodge something in means affixing or embedding.
  - beavault
- bearier - bear and barrier - I think it's clever.  Also NA, though less canada.  Bears = strength, barrier = blocking.  Don't like the meaning as much, but like the way it feels in my mouth.  Though would get confused for barrier when said outloud.

## System Components

The wallet consists of three distinct components with clear separation of concerns:

### Frontend

Translates raw data from Host/Plugin APIs to user interfaces. Cross-platform (Tauri, CLI, server-side). Defines and handles all UI interactions directly with plugins.
   - UX
   - UI Method Definitions
   - UI Security

### Host

Defines interface contracts, manages plugin lifecycle, handles routing, etc. Acts as trait registry and message router. Passes UI messages opaquely between plugins and frontend.
   - Network Stack
   - Persistent Data (storage)
   - Plugin routing
   - Opaque UI message forwarding
   - Permission Management

### Plugins

Implement host-defined interfaces. Handle all business logic including cryptography, network operations, and application-specific workflows.
   - Permission Granting & Revoking
   - User Authentication
   - Account Management
   - Transaction Management
   - Backups & Syncing

Plugins should be self-contained.  A core requirement will be avoiding plugin dependencies, where one plugin requires another to function.  Rather, plugin behavior should be implemented
statically and, when they need dependencies, should include them.  

## Update Requirements

**Plugin-only updates** handle workflow changes and new applications of existing primitives:
- New chains using existing models (EVM variants reusing Ethereum types)
- New signature generation methods (MPC, multi-sig wallets)
- New key management approach
- Complex workflows (bridging, social recovery, DeFi protocols)
- UI enhancements and analysis features

**Host updates required** only for new fundamental primitives:
- Proposal namespace changes or additions (https://specs.walletconnect.com/2.0/specs/clients/sign/namespaces) 
  - IE account abstraction
  - New chains not fitting existing namespaces
- New security boundaries (network access, USB device access)

This architecture maximizes extensibility while maintaining strong type safety and preventing ecosystem fragmentation through canonical chain specifications.

## API Versioning

### Backward-Compatible Changes with Enums

For maximum type safety and extensibility, all major data structures use tagged enums with `#[serde(other)]` Unknown variants. This approach handles both protocol evolution (new transaction types like EIP-1559) and multi-chain support naturally.

When new variants are added, old plugins receive `Unknown` and can gracefully degrade or warn users. New plugins handle the variants explicitly. JSON encoding remains stable through `#[serde(rename)]` while Rust code gets proper naming - for example, renaming `Transaction` to `Legacy` when EIP-1559 arrives.

Within enum variants, optional fields using `Option<T>` allow non-breaking additions of new protocol features. This provides two levels of compatibility: variant-level (new transaction types) and field-level (new transaction parameters).

### Example Pattern

```rust
#[serde(tag = "type")]
pub enum Transaction {
    #[serde(rename = "transaction")]
    Legacy { gas_price: String, ... },
    EIP1559 { max_fee_per_gas: String, ... },
    #[serde(other)]
    Unknown,
}
```

Breaking changes requiring major version bumps are reserved for fundamental architecture shifts, not protocol evolution.

## Routing Strategies

Each API request type has a defined routing strategy as part of its contract:

 - Singleton Routing: One plugin per resource (e.g., signing - routed to account owner)
 - Broadcast Routing: All capable plugins respond (e.g., risk analysis - collect all opinions)

For cases requiring user selection (e.g., account creation), the frontend lists available plugins and makes a singleton request to the chosen plugin.

```rust
pub enum RoutingStrategy {
    Singleton { owner_key: OwnershipKey },
    Broadcast { aggregation: AggregationStrategy },
}
```

## Security

### Permission requirements

Plugins will need to explicitly request access to **all** host endpoints.  Different host endpoints will have different security considerations (different levels of warnings), but all plugins will be allowed to access all endpoints assuming user permission is granted.

Permission can either:
- Permanently via a manifest file
- One-off for specific requests

Certain particularly sensitive permissions (IE `backup_import`, `backup_export`) may only be granted one-off.  This is up for the permission management program to decide.

**Frontend Permissions:** The frontend has unrestricted access to all host functionality since it presents the user interface. Any attempt to restrict frontend permissions would be meaningless - a malicious frontend could simply present fake UI to capture user input and steal permissions regardless.

### Storage

Plugins have access to hardware security module (HSM), encrypted, and plaintext storage.

- HSM storage: Stored encrypted with the user's authentication and on-device TEE or secure-enclave protections.  Suitable for private keys, seed phrases, or API keys.
- Encrypted storage: Stored encrypted with the user's authentication.  Can be moved from one device to another.  Suitable for wallet configuration, transaction history, or cached network data.
- Plaintext storage: Stored in plaintext.  Can be accessed without the user's authentication.  Suitable for addresses, balances, cache, or other non-sensitive data.

```rust
enum StorageScope {
    HSM,
    Encrypted,
    Plaintext
}
```

|                                   | `HSM` | `Encrypted` | `Plaintext` |
| --------------------------------- | ----- | ----------- | ----------- |
| Encrypted                         | Yes   | Yes         | No          |
| Portable                          | No    | Yes         | Yes         |
| Accessible without Authentication | No    | No          | Yes         |

Plugins may also implement alternative storage models (IE network-based) for their own purposes.

## Permissions

Plugins request permissions for three distinct categories:

### Handlers

Handlers are functions plugins implement for the host. They trigger when requests are made to perform some action. The vast majority of the host's API will be implemented by plugins. A single plugin can be registered for a given handler.
- JSON-RPC methods https://docs.metamask.io/wallet/reference/json-rpc-methods/
- CAIP-25 multi-chain methods https://github.com/ChainAgnostic/CAIPs/blob/main/CAIPs/caip-25.md
- Ethereum provider API https://docs.metamask.io/wallet/reference/provider-api/

Handlers can be scoped per-chain, per-account, or globally.  Depending on their scope, different handlers can be registered for different tasks.  For example, one might have multiple account handlers on a single chain for each account, or multiple chain handlers for different EVM chains.  

### Hooks

Hooks are functions plugins implement for the host. They trigger alongside handled requests.  The difference between handlers and hooks is that
1. While only a single handler can be registered per function, multiple hooks can be registered.
2. Handlers are expected to return a result, while hooks are expected to perform actions.

A handler might implement transaction signing or transmission, while a hook might attach to the `pre_eth_call` hook and check requests before they're called.
- `pre` and `post` hooks for most handlers.

### Requests

Requests are functions exposed by the host to plugins.  This includes the entire host's public API, plus a set of plugin-specific requests. 
 - All Handler functions
 - `plugin_*` namespace functions
 - Various requests for network requests, subscribing to events, 

## UI Architecture

UI should be like homeassistant or VScode - there should be standard "views" plugins can deal with (IE popup, page, card) and these should be combined together at the UI-level.  Non-UI interactions can be facilitated directly through the API.

## Performance Considerations

**WASM Overhead:** While WASM provides excellent security isolation, it introduces computational overhead.

**Plugin Loading:** Lazy loading and caching strategies will be essential for maintaining responsive UX, especially with many installed plugins.  Host's responsibility.

## Program Architecture

tlock will use wasmer for its wasm runtime, and with wasmer will use std-pipes for communication.  wasmer was selected because it supports a vast array of backends, including IOS (https://wasmer.io/posts/introducing-wasmer-v5), making it ideal for cross-platform development. Pipes were selected for communication because (a) they are very simple to implement, (b) can carry arbitrary data without needing manual memory shenanigans, and (c) allow the host and plugin to naturally implement async waits for each other. 

## Development Experience

**Plugin SDK:** A comprehensive Rust SDK will provide type-safe host bindings, UI abstractions, and testing utilities to streamline plugin development.

**Testing Framework:** Plugins need isolated testing environments that mock host services and different frontend types.