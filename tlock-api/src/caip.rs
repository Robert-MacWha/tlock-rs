use std::{fmt::Display, str::FromStr};

use alloy::primitives::Address;
use serde::{Deserialize, Serialize};

// ---------- ChainId ----------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChainId {
    Evm(Option<u64>),
    Custom {
        namespace: String,
        reference: Option<String>,
    },
}

impl ChainId {
    pub fn new(namespace: String, reference: Option<String>) -> Self {
        if namespace == "eip155" {
            let chain_id = reference.and_then(|r| if r == "_" { None } else { r.parse().ok() });
            return Self::Evm(chain_id);
        }
        Self::Custom {
            namespace,
            reference,
        }
    }

    pub fn new_evm(chain_id: u64) -> Self {
        Self::Evm(Some(chain_id))
    }

    pub fn namespace(&self) -> &str {
        match self {
            Self::Evm(_) => "eip155",
            Self::Custom { namespace, .. } => namespace,
        }
    }

    pub fn reference(&self) -> Option<String> {
        match self {
            Self::Evm(Some(id)) => Some(id.to_string()),
            Self::Evm(None) => Some("_".to_string()),
            Self::Custom { reference, .. } => reference.clone(),
        }
    }
}

impl FromStr for ChainId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        match parts.as_slice() {
            ["eip155", reference] => {
                let chain_id = if *reference == "_" {
                    None
                } else {
                    Some(
                        reference
                            .parse()
                            .map_err(|_| format!("Invalid EVM chain ID: {}", reference))?,
                    )
                };
                Ok(Self::Evm(chain_id))
            }
            [namespace, reference] => {
                let reference = if *reference == "_" {
                    None
                } else {
                    Some(reference.to_string())
                };
                Ok(Self::Custom {
                    namespace: namespace.to_string(),
                    reference,
                })
            }
            [_namespace] => {
                Err("Chain ID must have a reference (use '_' for wildcard)".to_string())
            }
            _ => Err(format!("Invalid chain ID format: {}", s)),
        }
    }
}

impl Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Evm(Some(id)) => write!(f, "eip155:{}", id),
            Self::Evm(None) => write!(f, "eip155:_"),
            Self::Custom {
                namespace,
                reference,
            } => {
                if let Some(r) = reference {
                    write!(f, "{}:{}", namespace, r)
                } else {
                    write!(f, "{}:_", namespace)
                }
            }
        }
    }
}

impl Serialize for ChainId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ChainId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ChainId::from_str(&s).map_err(serde::de::Error::custom)
    }
}

// ---------- AccountId ----------

/// CAIP-10 Account ID.
///
/// https://chainagnostic.org/CAIPs/caip-10
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountId {
    pub chain_id: ChainId,
    pub address: AccountAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AccountAddress {
    Evm(Address),
    Custom(String),
}

impl AccountId {
    pub fn new(chain_id: ChainId, address: Address) -> Self {
        Self {
            chain_id,
            address: AccountAddress::Evm(address),
        }
    }

    pub fn new_evm(chain_id: u64, address: Address) -> Self {
        Self {
            chain_id: ChainId::Evm(Some(chain_id)),
            address: AccountAddress::Evm(address),
        }
    }

    pub fn chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    pub fn address(&self) -> &AccountAddress {
        &self.address
    }

    pub fn as_evm_address(&self) -> Option<Address> {
        match self.address {
            AccountAddress::Evm(addr) => Some(addr),
            _ => None,
        }
    }

    pub fn try_into_evm_address(&self) -> Result<Address, String> {
        match &self.chain_id {
            ChainId::Evm(_) => match self.address {
                AccountAddress::Evm(addr) => Ok(addr),
                AccountAddress::Custom(_) => Err("Address is not EVM format".to_string()),
            },
            _ => Err(format!(
                "Unsupported chain namespace: {}",
                self.chain_id.namespace()
            )),
        }
    }
}

impl FromStr for AccountId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Format: "eip155:1:0x..."
        let parts: Vec<&str> = s.splitn(3, ':').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid account ID format: {}", s));
        }

        let chain_str = format!("{}:{}", parts[0], parts[1]);
        let chain_id = ChainId::from_str(&chain_str)?;

        let address = if parts[0] == "eip155" {
            let addr = parts[2]
                .parse::<Address>()
                .map_err(|e| format!("Invalid EVM address: {}", e))?;
            AccountAddress::Evm(addr)
        } else {
            AccountAddress::Custom(parts[2].to_string())
        };

        Ok(AccountId { chain_id, address })
    }
}

impl Display for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.address {
            AccountAddress::Evm(addr) => write!(f, "{}:{:#x}", self.chain_id, addr),
            AccountAddress::Custom(addr) => write!(f, "{}:{}", self.chain_id, addr),
        }
    }
}

impl Serialize for AccountId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for AccountId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        AccountId::from_str(&s).map_err(serde::de::Error::custom)
    }
}

// ---------- AssetId ----------

/// CAIP-19 Asset ID
///
/// https://chainagnostic.org/CAIPs/caip-19
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetId {
    pub chain_id: ChainId,
    pub asset: AssetType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetType {
    Slip44(u32),
    Erc20(Address),
    Custom {
        namespace: String,
        reference: String,
    },
}

impl AssetId {
    pub fn new(chain_id: ChainId, namespace: String, reference: String) -> Self {
        let asset = match namespace.as_str() {
            "slip44" => AssetType::Slip44(reference.parse().unwrap_or(0)),
            "erc20" => {
                if let Ok(addr) = reference.parse() {
                    AssetType::Erc20(addr)
                } else {
                    AssetType::Custom {
                        namespace,
                        reference,
                    }
                }
            }
            _ => AssetType::Custom {
                namespace,
                reference,
            },
        };
        Self { chain_id, asset }
    }

    pub fn eth(chain_id: u64) -> Self {
        Self {
            chain_id: ChainId::Evm(Some(chain_id)),
            asset: AssetType::Slip44(60),
        }
    }

    pub fn erc20(chain_id: u64, contract: Address) -> Self {
        Self {
            chain_id: ChainId::Evm(Some(chain_id)),
            asset: AssetType::Erc20(contract),
        }
    }

    pub fn chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    pub fn namespace(&self) -> &str {
        match &self.asset {
            AssetType::Slip44(_) => "slip44",
            AssetType::Erc20(_) => "erc20",
            AssetType::Custom { namespace, .. } => namespace,
        }
    }

    pub fn reference(&self) -> String {
        match &self.asset {
            AssetType::Slip44(coin) => coin.to_string(),
            AssetType::Erc20(addr) => format!("{:#x}", addr),
            AssetType::Custom { reference, .. } => reference.clone(),
        }
    }

    pub fn try_into_erc20_address(&self) -> Result<Address, String> {
        match &self.chain_id {
            ChainId::Evm(_) => match &self.asset {
                AssetType::Erc20(addr) => Ok(*addr),
                _ => Err(format!("Asset is not ERC20: {}", self.namespace())),
            },
            _ => Err(format!(
                "Unsupported chain namespace: {}",
                self.chain_id.namespace()
            )),
        }
    }
}

impl FromStr for AssetId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Format: "eip155:1/erc20:0x..." or "eip155:1/slip44:60"
        let parts: Vec<&str> = s.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid asset ID format: {}", s));
        }

        let chain_id = ChainId::from_str(parts[0])?;

        let asset_parts: Vec<&str> = parts[1].splitn(2, ':').collect();
        if asset_parts.len() != 2 {
            return Err(format!("Invalid asset format: {}", parts[1]));
        }

        let asset = match asset_parts[0] {
            "slip44" => {
                let coin = asset_parts[1]
                    .parse()
                    .map_err(|_| format!("Invalid slip44 coin type: {}", asset_parts[1]))?;
                AssetType::Slip44(coin)
            }
            "erc20" => {
                let addr = asset_parts[1]
                    .parse::<Address>()
                    .map_err(|e| format!("Invalid ERC20 address: {}", e))?;
                AssetType::Erc20(addr)
            }
            namespace => AssetType::Custom {
                namespace: namespace.to_string(),
                reference: asset_parts[1].to_string(),
            },
        };

        Ok(AssetId { chain_id, asset })
    }
}

impl Display for AssetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let asset_str = match &self.asset {
            AssetType::Slip44(coin) => format!("slip44:{}", coin),
            AssetType::Erc20(addr) => format!("erc20:{:#x}", addr),
            AssetType::Custom {
                namespace,
                reference,
            } => format!("{}:{}", namespace, reference),
        };
        write!(f, "{}/{}", self.chain_id, asset_str)
    }
}

impl Serialize for AssetId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for AssetId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        AssetId::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_id_serde() {
        let asset = AssetId::erc20(
            1,
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                .parse()
                .unwrap(),
        );

        let json = serde_json::to_string(&asset).unwrap();
        assert_eq!(
            json,
            "\"eip155:1/erc20:0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48\""
        );

        let parsed: AssetId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.to_string(), asset.to_string());
    }

    #[test]
    fn test_account_id_serde() {
        let account = AccountId::new_evm(
            1,
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"
                .parse()
                .unwrap(),
        );

        let json = serde_json::to_string(&account).unwrap();
        assert_eq!(
            json,
            "\"eip155:1:0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed\""
        );

        let parsed: AccountId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.to_string(), account.to_string());
    }

    #[test]
    fn test_wildcard_chain_id() {
        let chain = ChainId::Evm(None);
        assert_eq!(chain.to_string(), "eip155:_");

        let parsed: ChainId = "eip155:_".parse().unwrap();
        assert_eq!(parsed, chain);
    }
}
