This plugin provides an revm-based Ethereum provider for Tlock. It allows the user to fork from a specified EVM network at a given block and interact with the forked state, mining new blocks as needed.

# Architecture

The plugin uses the new state management provided by the Tlock PDK so it works concurrently without disputed state.  Essentially, it:

1. Has a base `alloy_db` that queries data from the selected network at the fork block.
2. Wraps that in a custom caching `cache_db` that stores all data queried from the network in key-value storage.
   1. Use a custom `cache_db` instead of `revm::CacheDB` because we want the cache to store key-value pairs instead of a monolithic cache struct. Using KV means it's much easier for concurrent execution to share the cache without blocking each other.
3. Wraps that in a list of `layered_db`s, one for each block mined after the fork.  Each `layered_db` contains only the state changes for that block, and are append-only.  Basically like a real blockchain.
   1. I expect forks stay small, so keeping all layers should be fine.  If needed we could flatten older layers into the cache_db to save memory later.

The `Chain` struct manages these layers and provides a basic interface for querying and updating state.  That can then be used by main to implement the eth-provider methods. 