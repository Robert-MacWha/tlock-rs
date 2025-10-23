use alloy::primitives::{Address, ChainId};
use serde::{Deserialize, Serialize};

// TODO: Consider making these stricter or enums to prevent invalid IDs.
pub type AssetNamespace = String;
pub type AssetReference = String;

/// CAIP-10 Account ID.
///
/// https://chainagnostic.org/CAIPs/caip-10
///
/// TODO: Add support for other namespaces (solana, bitcoin, etc)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(ChainId, Address);

impl AccountId {
    pub fn new(chain_id: ChainId, address: Address) -> Self {
        Self(chain_id, address)
    }

    pub fn chain_id(&self) -> &ChainId {
        &self.0
    }

    pub fn address(&self) -> &Address {
        &self.1
    }

    pub fn to_string(&self) -> String {
        format!("{}:{}", self.0, self.1)
    }
}

/// CAIP-19 Asset ID
///
/// https://chainagnostic.org/CAIPs/caip-19
///
/// TODO: Add support for other namespaces (solana, bitcoin, etc)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetId(ChainId, AssetNamespace, AssetReference);

impl AssetId {
    pub fn new(chain_id: ChainId, namespace: AssetNamespace, reference: AssetReference) -> Self {
        Self(chain_id, namespace, reference)
    }

    pub fn chain_id(&self) -> &ChainId {
        &self.0
    }

    pub fn namespace(&self) -> &str {
        &self.1
    }

    pub fn reference(&self) -> &str {
        &self.2
    }

    pub fn to_string(&self) -> String {
        format!("{}:{}:{}", self.0, self.1, self.2)
    }
}
