# Tlock-rs

Tlock is designed as a modular-focused wallet framework.  It is designed to, as much as possible, get out of the way while providing a framework that allows plugins to securely and effectively perform tasks.  Its priorities are:

1. Modularity. Unless there's a very good reason, modular plugins should be responsible for all functionality.

2. Security. Modularity cannot mean a decrease in security for the user's machine, privacy, or money.

3. Portability. Tlock should be usable across arbitrary platforms, with arbitrary public interfaces.

This document contains a system overview.
- See [status-quo.md](./docs/status-quo.md) for problems I have with the current status quo.
- See [design-considerations.md](./docs/design-considerations.md) for problems I have this this proposal.


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

|           | HSM | Encrypted | Plaintext |
| --------- | --- | --------- | --------- |
| Encrypted | Yes | Yes       | No        |
| Portable  | No  | Yes       | Yes       |

Plugins may also implement alternative storage models (IE network-based) for their own purposes.

## Permissions

Plugins request permissions for two distinct categories:

### Actions
Actions are functions plugins can call on the host.  Calling actions will either require specifying a plugin ID to call a single action, or will call all plugins and return a list result. 

| Permission                  | Description                                                       |
| --------------------------- | ----------------------------------------------------------------- |
| `eip155:account_create`     | Create new accounts for EVM chains                                |
| `eip155:account_list`       | Lists all accounts for EVM chains                                 |
| `eip155:sign`               | Sign messages and transactions                                    |
| `wallet:encrypt`            | Encrypt messages using account keys                               |
| `wallet:decrypt`            | Decrypt messages using account keys                               |
| `wallet:permission_get`     | Gets the plugin's permission status                               |
| `wallet:permission_grant`   | Grant permissions to other plugins                                |
| `wallet:permission_revoke`  | Revoke permissions from plugins                                   |
| `wallet:backup_export`      | Export wallet backup data from the host (encrypted)               |
| `wallet:backup_import`      | Import wallet backup data into the host (encrypted)               |
| `wallet:storage_read`       | Read from plugin storage (scoped to the plugin)                   |
| `wallet:storage_write`      | Write to plugin storage (scoped to the plugin)                    |
| `network:http_request`      | Make HTTP requests to external services (url scoping)             |
| `network:websocket_connect` | Establish WebSocket connections (url scoping)                     |
| `ui:alert`                  | Alerts to the UI to an important event.  Universal across all UIs |

Something for push notifications
Something for account abstraction
Lots for different chain namespaces

### Handler
Handlers are functions plugins can implement for the host. Handler patterns:

| Handler                 | Description                        |
| ----------------------- | ---------------------------------- |
| `eip155:account_create` | Create new accounts for EVM chains |
| `eip155:account_list`   | Lists all accounts for EVM chains  |
| `eip155:sign`           | Sign messages and transactions     |
| `ui:*`                  | Generic UI handler                 |

### Hooks
Hooks are observable events plugins can connect to that do not include a 

| Permission                     | Description                             |
| ------------------------------ | --------------------------------------- |
| `eip155:pre_sign`              | Execute before signing operations       |
| `eip155:post_sign`             | Execute after signing operations        |
| `eip155:transaction_broadcast` | Observe when transactions are broadcast |
| `wallet:plugin_installed`      | Observe when new plugins are installed  |
| `wallet:permission_changed`    | Observe permission changes              |

**Note:** UI capabilities (`ui:*`) are defined entirely by each frontend and passed opaquely through the host. While the specific UI methods and types are frontend-defined, plugins must still request the `ui:*` permission to access any UI functionality.

## UI Architecture

All UI communication uses frontend-scoped enums to maintain type safety while allowing frontend flexibility. Plugins can create unified enums that combine multiple frontend types for cleaner handling.

### Example Frontend Enums

```rust
// CLI Frontend
#[derive(Serialize, Deserialize)]
pub enum CliUiRequest {
    GetHomepage {},
    Input { key: String, value: Option<String> },
}
```

```rust
// Web Frontend  
#[derive(Serialize, Deserialize)]
pub enum WebUiRequest {
    GetHomepage {},
    Input { key: String, value: Option<String> },
}
```

### Plugin Unified Enum Pattern

Plugins can create their own enums that combine supported frontend types:

```rust
// Plugin's unified UI enum
#[derive(Serialize, Deserialize)]
pub enum UiRequest {
    Cli(CliUiRequest),
    Web(WebUiRequest),
    #[serde(other)]
    Unsupported,
}

fn handle_ui_request(request: UiRequest) -> Result<UiResponse, Error> {
    match request {
        UiRequest::Cli(cli_req) => handle_cli_ui(cli_req),
        UiRequest::Web(web_req) => handle_web_ui(web_req),
        UiRequest::Unsupported => Err(Error::UnsupportedUiType),
    }
}
```

## Performance Considerations

**WASM Overhead:** While WASM provides excellent security isolation, it introduces computational overhead.

**Plugin Loading:** Lazy loading and caching strategies will be essential for maintaining responsive UX, especially with many installed plugins.  Host's responsibility.

## Development Experience

**Plugin SDK:** A comprehensive Rust SDK will provide type-safe host bindings, UI abstractions, and testing utilities to streamline plugin development.

**Testing Framework:** Plugins need isolated testing environments that mock host services and different frontend types.