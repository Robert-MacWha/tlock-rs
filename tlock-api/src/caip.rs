use alloy_primitives::{Address, ChainId};
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

/// CAIP-19 Asset ID
///
/// https://chainagnostic.org/CAIPs/caip-19
///
/// TODO: Add support for other namespaces (solana, bitcoin, etc)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetId(ChainId, AssetNamespace, AssetReference);
