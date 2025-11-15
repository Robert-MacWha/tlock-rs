use std::fmt::Display;

use alloy::primitives::Address;
use serde::{Deserialize, Serialize};

// TODO: Consider making these stricter or enums to prevent invalid IDs.
pub type AssetNamespace = String;
pub type AssetReference = String;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChainId {
    namespace: String,
    reference: Option<String>,
}

impl ChainId {
    pub fn new(namespace: String, reference: Option<String>) -> Self {
        Self {
            namespace,
            reference,
        }
    }

    pub fn new_evm(chain_id: u64) -> Self {
        Self {
            namespace: "eip155".to_string(),
            reference: Some(chain_id.to_string()),
        }
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn reference(&self) -> Option<&str> {
        self.reference.as_deref()
    }
}

impl Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref reference) = self.reference {
            write!(f, "{}:{}", self.namespace, reference)
        } else {
            write!(f, "{}:_", self.namespace)
        }
    }
}

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

    pub fn try_into_evm_address(&self) -> Result<Address, String> {
        if self.0.namespace() != "eip155" {
            return Err(format!(
                "Unsupported chain namespace: {}",
                self.0.namespace()
            ));
        }
        Ok(self.1)
    }
}

impl Display for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.0, self.1)
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
}

impl Display for AssetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.0, self.1, self.2)
    }
}
