# Lodgelock

⚠️ Pre-alpha: Not audited, not production-ready, use at your own risk! ⚠️

Lodgelock is designed as a modular-first wallet framework. It aims to empower users, developers, and the broader web3 ecosystem by providing a secure, extensible, and user-centric wallet platform.

Lodgelock is designed around three core ideals:

1. **Wallets for self-sovereignty**: Wallets are tools for users to manage their money, identity, and data. They should empower users to hold ownership over their digital lives, providing full control and autonomy.
2. **Wallets for security**: Wallets act as guardians of users' assets and information. They are a critical single point of failure and must prioritize security, ensuring that funds, privacy, and integrity are maintained at all times.
3. **Wallets for modularity**: Wallets should act as the kernel for web3 applications. They should provide a secure and unopinionated platform that manages security and resources, leaving the 'user-space' features entirely implemented by modular plugins to the user's discretion.

## Docs

- [Architecture Overview](./docs/ARCHITECTURE.md)
- [Plugin Development Guide](./docs/PLUGIN_DEVELOPMENT.md)
- [Security Model](./docs/SECURITY.md)

## How it Works

Lodgelock is built around a secure host that loads and manages untrusted arbitrary plugins. The host provides core services like storage, networking, and plugin management. Plugins implement wallet functionality by registering entities with the host. This way, plugins can be installed independently and provide arbitrary features (key management, blockchain providers, DeFi protocols, UI components, etc) without requiring any host updates.

## Status Quo

The current wallet landscape is dominated by monolithic walled gardens that bundle a fixed set of features and applications. DeFi's origin as websites has been a saving grace, allowing users to access a broader ecosystem of applications. However, wallets have not embraced this modularity, instead opting for closed systems that limit user choice and stifle innovation.

This lack of modularity has created a dangerous example of the **Power of Defaults**. Wallets decree what features and applications are present within their walls, shaping behavior and experience for millions. For a prime example, simply look at the exchange rates provided by Metamask's built-in swap versus uniswap or 1inch. When the gateway to web3 is a profit-seeking entity, the default becomes a toll-bridge rather than an open road.

Wallets also suffer from **Extractive Incentives**. When the wallet controls the default experience, it is incentivised to prioritize features that enrich itself over those that benefit users. Consider Phantom integrating kalshi prediction markets into their app, adding gambling features to the homepage of a financial application without user consent, warning, age verification, or an option to disable it. While users should undoubtedly have the freedom to have such features, they should not be forced upon them.

## Getting Started

Lodgelock is currently in early pre-alpha development. (Web demo coming soon).

To build from source, Lodgelock uses [Nix](https://nixos.org/) to manage dependencies and build environments. Alternatively, you can manually install the required dependencies listed in `shell.nix`.

(Devcontainer coming soon)

```bash
git clone git@github.com:Robert-MacWha/tlock-rs.git
cd tlock-rs
nix-shell # Enter nix shell with dependencies. Alternatively, install the listed dependencies manually.
make plugins # Build all plugins
cd frontend

# Concurrently run tailwindcss watcher, dioxus dev server, and chrome with various security features to allow SharedArrayBuffer
dev # Provided by shell.nix
```

## Roadmap

See the [project board](https://github.com/Robert-MacWha/tlock-rs/issues) for current tasks and progress.

## License

This project is currently unlicensed while in pre-alpha development.

## Open Questions

- State mechanism. Currently implementing the plans outlined in [state.md](./docs/state.md) which is essentially a key-value mutexed storage. This allows plugins to store state and prevents state corruption from concurrent access. However, it also limits concurrent access to state which may be a future bottleneck.
- Cross-chain abstractions. Using CAIP standards for chain, account, and asset IDs. Chain-specific domains (e.g. eth-provider, coordinator) are currently chain-specific. Should these be abstracted, or is it better to create new domains for different chains (and thus harm the 90/10 rule)?
- Will plugin management UX be acceptable?
  - Managing plugins requires non-trivial user comprehension of what the plugins are and how they interact. Conceptually this is similar to browser extensions, homeassistant plugins, or desktop environments. However, wallets are security-critical software and users should be less willing to tinker.
  - In actual distributions, the host should ship with a curated set of plugins by default (some enabled, some optional) to provide a good out-of-the-box experience.
    - Similar to the web browser demo I've built for the alpha.
  - I'm considering adding more fine-grained domains (e.g. "swap", "stake", "bridge") to allow plugins to be better categorized and for generic UIs to be built around them. This way users could easily find and install plugins for specific features, for example if they notice their swap plugin is missing a certain token they want or they want to try a different staking provider.
