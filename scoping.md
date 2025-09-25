Essentially, we act like HA domains.  Different objects implement different namespaces, and each namespace impl is tightly scoped via caip (and probably also some other stuff).

Across different caip-2 namspaces (IE eip155, bip122), things are split via different traits. So routing is trivial.
    - This is the main conceptual difference.  caip namespace IDs are like the HA entity IDs, and automatic routing is highly prioritized.  We want to make sure that the right resource is automatically selected as often as possible.

Across different chains within the same namespace, routing gets a little more complex.  Basically, each interface instance
is scoped to its own caip-2, caip-10, caip-19, etc scope.  Then when I'm performing actions I'll lookup the most specific
provider which implements the function and call it.

So a Eip155Provider might implement most of the json-rpc spec, providing access to an "eth node".  It will be scoped to a 
caip-2 chainId and different instances of (potentially) the same plugin will run concurrently across multiple chains. 
    - Alternatively, I could have a single instance of a plugin running across multiple chains.  But having a single instance per identifier seems simplest and similar to HA's implementation

A Eip155Keyring might implement methods for signing and sending transactions, and be scoped to a caip-10 account id.  That means
it's both on a specific chain and at a specific address.
    - This also means methods change.  I'll need to translate from the generic `personal_sign(message, address)` into (a) routing to the correct Keyring, and (b) removing the address from the request (or maybe not - just keep it in cause why not?  I see no harm).
    - I might also want to support generics here ala caip-363.  But I think I could safely worry about that *later*

For methods with overlap (ie eth_balance for example), both Eip155Provider and Eip155Keyring might provide impls.  But since Eip155Keyring is more specific for accounts it owns, it'll be selected.  This can happen automagically when selecting different impls.

This approach means I need a much more significant glue layer between the external json-rpc and the internal ones.  That's unfortunate, but complexity should be limited to a few specific functions.

We also need to make mappings 1:1 - there can only be a single provider for a given caip-2 id, or a single keyring for a given caip-10 id.
    - But we'll also want some cases where many instances can exist I think. For example, notifiers will exist globally / across all chains, and I might want many of those
      - Or maybe not - I'm not sure what the best approach is here.  This runs into a fundemental difference between tlock and ha - tlock needs to route everything automatically, while ha relies on users to establish connections.  this means tlock inherintly needs a stricter system so it can establish those connections.