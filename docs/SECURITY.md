# Security

!!! Pre-alpha: use at your own risk !!!

## Threat Model

Lodgelock assumes that plugins are untrusted and potentially malicious. The host must limit the capabilities of plugins to prevent them from compromising user security or privacy. The primary threats include:
    - **Data Exfiltration**: Plugins attempting to steal sensitive user data (private keys, transaction history, personal information).
    - **Fund Theft**: Plugins attempting to drain user assets.
    - **Unauthorized Transactions**: Plugins trying to initiate transactions without user consent.
    - **Denial of Service**: Plugins consuming excessive resources or crashing the host.

Malicious plugins CAN:
    - Request permissions from the host
    - Once permission is granted, call host functions

Malicious plugins CANNOT:
    - Access any resources without explicit permission from the host
    - Bypass the JSON-RPC interface to communicate directly with the network or file system
    - Interact with other plugins directly
    - Run indefinitely

In order for plugins to carry out these threats, they must be installed by the user. Therefore, user education and trust in plugin sources is also a critical component of the overall security model.

## Plugin Sandboxing

Plugins are executed in a sandboxed WebAssembly (WASM) environment with no direct access to the network, file system, or user machine. All interactions between plugins and the host are mediated through the JSON-RPC over STDIO interface. This allows the host to:
    1. Enforce permission controls on what individual plugins can and cannot do.
    2. Monitor and log all interactions for auditing purposes.
    3. Isolate plugins from each other.
    4. Easily terminate or restart misbehaving plugins.

### Permission Model

!!! Permission Model is currently under development and not fully implemented. !!!

Before calling host functions plugins must request permission from the host. The host will present the user with a permission prompt which can then be approved or rejected. Permissions are per-method and per-entity. For example, a plugin may request permission to call `vault_get_assets` on a user's `eoa-vault-1`. Permissions are also temporally granular, meaning they can be granted for a single call, for the duration of a plugin session, or permanently. Permissions can be revoked at any time by the user or the host.

Different permissions will have different levels of associated risk. Local read permissions (e.g. `vault_get_assets`, `page_on_load`, `host_set_state`) are low-risk, while permissions that allow fund transfers or enable networking access (e.g. `coordinator_get_session`, `vault_withdraw`) are high-risk. 

### Plugin Distribution

Plugins should be distributed through trusted channels to minimize the risk of malicious code. This may include:
    - Plugin repositories (similar to app stores) with vetting processes
    - Code signing and verification mechanisms
    - Explicit "Developer Mode" requirement for installing unsigned or unvetted plugins

Importantly, users should always be capable of installing arbitrary plugins at their own risk. Lodgelock should never restrict user choice, but rather provide safeguards and information to help users make informed decisions.
