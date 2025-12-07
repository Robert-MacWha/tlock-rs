use std::{
    fmt::{self, Display},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct VaultId(Uuid);

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PageId(Uuid);

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct EthProviderId(Uuid);

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct CoordinatorId(Uuid);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum EntityId {
    Vault(VaultId),
    Page(PageId),
    EthProvider(EthProviderId),
    Coordinator(CoordinatorId),
}

impl Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntityId::Vault(vault_id) => write!(f, "{}", vault_id),
            EntityId::Page(page_id) => write!(f, "{}", page_id),
            EntityId::EthProvider(eth_provider_id) => write!(f, "{}", eth_provider_id),
            EntityId::Coordinator(coordinator_id) => write!(f, "{}", coordinator_id),
        }
    }
}

impl Serialize for EntityId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for EntityId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        if let Ok(vault_id) = VaultId::from_str(&s) {
            return Ok(EntityId::Vault(vault_id));
        }
        if let Ok(page_id) = PageId::from_str(&s) {
            return Ok(EntityId::Page(page_id));
        }
        if let Ok(provider_id) = EthProviderId::from_str(&s) {
            return Ok(EntityId::EthProvider(provider_id));
        }
        if let Ok(coordinator_id) = CoordinatorId::from_str(&s) {
            return Ok(EntityId::Coordinator(coordinator_id));
        }

        Err(serde::de::Error::custom(format!(
            "Invalid EntityId string: {}",
            s
        )))
    }
}

// TODO: Setup macros for these repetitive impls

impl VaultId {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for VaultId {
    fn default() -> Self {
        VaultId(Uuid::new_v4())
    }
}

impl Display for VaultId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vault:{}", self.0)
    }
}

impl FromStr for VaultId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix("vault:").unwrap_or(s);
        let uuid = Uuid::from_str(s)?;
        Ok(VaultId(uuid))
    }
}

impl From<VaultId> for EntityId {
    fn from(vault_id: VaultId) -> Self {
        EntityId::Vault(vault_id)
    }
}

impl PageId {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for PageId {
    fn default() -> Self {
        PageId(Uuid::new_v4())
    }
}

impl Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "page:{}", self.0)
    }
}

impl FromStr for PageId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix("page:").unwrap_or(s);
        let uuid = Uuid::from_str(s)?;
        Ok(PageId(uuid))
    }
}

impl From<PageId> for EntityId {
    fn from(page_id: PageId) -> Self {
        EntityId::Page(page_id)
    }
}

impl EthProviderId {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for EthProviderId {
    fn default() -> Self {
        EthProviderId(Uuid::new_v4())
    }
}

impl Display for EthProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "eth_provider:{}", self.0)
    }
}

impl FromStr for EthProviderId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix("eth_provider:").unwrap_or(s);
        let uuid = Uuid::from_str(s)?;
        Ok(EthProviderId(uuid))
    }
}

impl From<EthProviderId> for EntityId {
    fn from(eth_provider_id: EthProviderId) -> Self {
        EntityId::EthProvider(eth_provider_id)
    }
}

impl CoordinatorId {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for CoordinatorId {
    fn default() -> Self {
        CoordinatorId(Uuid::new_v4())
    }
}

impl Display for CoordinatorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "coordinator:{}", self.0)
    }
}

impl FromStr for CoordinatorId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix("coordinator:").unwrap_or(s);
        let uuid = Uuid::from_str(s)?;
        Ok(CoordinatorId(uuid))
    }
}

impl From<CoordinatorId> for EntityId {
    fn from(coordinator_id: CoordinatorId) -> Self {
        EntityId::Coordinator(coordinator_id)
    }
}
