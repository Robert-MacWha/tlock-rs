# Plugins

This document outlines each of the plugins currently included in the Lodgelock demo. For more information on the overall architecture or the different domains, see the [Architecture Overview](./ARCHITECTURE.md).

## revm-provider

Entities: `eth-provider`, `page`

The `revm-provider` plugin provides an `eth-provider` backed by [revm](https://github.com/bluealloy/revm). This allows for fast, local Ethereum RPC calls that can fork from mainnet (or any other EVM chain) at a specific block. Very useful for development and demonstrations.

A `page` is also provided that shows some basic information about the current fork and allows the user to access cheatcodes provided by revm.

## eoa-vault

Entities: `vault`, `page`

The `eoa-vault` plugin provides a simple vault backed by a local EOA generated from a random private key. It is functionally the simplest possible vault implementation.

For demonstration purposes a `page` is also provided to show the current address, private key, and balance of the vault. In general vaults should not create their own UI pages, but rather rely on a separate UI plugin to provide a unified interface across all vaults.

## staking

Entities: `page`, `vault`

The `staking` plugin is a simple staking dapp that can stake and unstake ETH into a custodial staking provider. It provides a `page` UI for interacting with the staking functionality.

Since staking is essentially just another type of vault, the plugin also provides a `vault` entity that represents the staked position. This vault could be used by other plugins to interact with the staked ETH, or simply used by a UI plugin to show the user's staked assets alongside their other vaults.

## uniswap-v2

Entities: `page`

The `uniswap-v2` plugin provides a simple swap interface using Uniswap V2 contracts. It provides a `page` entity that allows users to swap tokens using their connected vaults.

## eoa-coordinator

Entities: `coordinator`, `page`

The `eoa-coordinator` plugin provides a `coordinator` entity that acts as an atomic-ish intermediary between dapps and vaults. This coordinator is fairly basic and simply forwards requests from dapps to the connected vaults, ensuring if any request fails all assets are returned to their original vaults.

For more information on coordinators, see the [tlock-api](../crates/tlock-api/src/lib.rs) `coordinator` module, or the [vault_architecture.md](./internal/vault_architecture.md) document. 