use alloy_primitives::Address;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChainId {
    namespace: String,
    reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountId {
    chain_id: ChainId,
    address: String,
}

impl Serialize for ChainId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = format!(
            "{}:{}",
            self.namespace,
            self.reference.as_ref().unwrap_or(&"_".into())
        );
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for ChainId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(serde::de::Error::custom("Invalid CAIP-2 format"));
        }
        let namespace = parts[0].to_string();
        let reference = if parts[1] == "_" {
            None
        } else {
            Some(parts[1].to_string())
        };
        Ok(ChainId {
            namespace,
            reference,
        })
    }
}

impl Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}",
            self.namespace,
            self.reference.as_ref().unwrap_or(&"_".into())
        )
    }
}

impl Serialize for AccountId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = format!("{}:{}", self.chain_id, self.address);
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for AccountId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() < 3 {
            return Err(serde::de::Error::custom("Invalid CAIP-10 format"));
        }
        let namespace = parts[0].to_string();
        let reference = if parts[1] == "_" {
            None
        } else {
            Some(parts[1].to_string())
        };
        let address = parts[2].to_string();
        Ok(AccountId {
            chain_id: ChainId {
                namespace,
                reference,
            },
            address,
        })
    }
}

impl Display for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.chain_id, self.address)
    }
}

impl From<&alloy_primitives::ChainId> for ChainId {
    fn from(value: &alloy_primitives::ChainId) -> Self {
        ChainId {
            namespace: "eip155".to_string(),
            reference: Some(value.to_string()),
        }
    }
}

impl TryFrom<&ChainId> for alloy_primitives::ChainId {
    type Error = &'static str;

    fn try_from(value: &ChainId) -> Result<Self, Self::Error> {
        if value.namespace != "eip155" {
            return Err("Unsupported namespace");
        }

        let reference: u64 = value
            .reference
            .as_ref()
            .ok_or("Missing reference for eip155")?
            .parse()
            .map_err(|_| "Invalid reference format")?;

        Ok(reference)
    }
}

impl TryFrom<&AccountId> for alloy_primitives::Address {
    type Error = &'static str;

    fn try_from(value: &AccountId) -> Result<Self, Self::Error> {
        if value.chain_id.namespace != "eip155" {
            return Err("Unsupported namespace");
        }

        value.address.parse().map_err(|_| "Invalid address format")
    }
}
