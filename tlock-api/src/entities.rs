use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

use crate::domains::Domain;

/// Entities are uniquely identified registerable objects in tlock that act as
/// instances of a domain implemented by a particular plugin.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityId {
    Vault(VaultId),
    Page(PageId),
    EthProvider(EthProviderId),
}

impl EntityId {
    pub fn domain(&self) -> Domain {
        match self {
            EntityId::Vault(_) => Domain::Vault,
            EntityId::Page(_) => Domain::Page,
            EntityId::EthProvider(_) => Domain::EthProvider,
        }
    }
}

impl Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntityId::Vault(v) => v.fmt(f),
            EntityId::Page(p) => p.fmt(f),
            EntityId::EthProvider(e) => e.fmt(f),
        }
    }
}

impl From<VaultId> for EntityId {
    fn from(id: VaultId) -> Self {
        EntityId::Vault(id)
    }
}

impl From<PageId> for EntityId {
    fn from(id: PageId) -> Self {
        EntityId::Page(id)
    }
}

impl From<EthProviderId> for EntityId {
    fn from(id: EthProviderId) -> Self {
        EntityId::EthProvider(id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VaultId(String);

impl VaultId {
    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn as_entity_id(&self) -> EntityId {
        EntityId::Vault(self.clone())
    }
}

impl Display for VaultId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vault:{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PageId(String);

impl PageId {
    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn as_entity_id(&self) -> EntityId {
        EntityId::Page(self.clone())
    }
}

impl Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "page:{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EthProviderId(String);

impl EthProviderId {
    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn as_entity_id(&self) -> EntityId {
        EntityId::EthProvider(self.clone())
    }
}

impl Display for EthProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ethprovider:{}", self.0)
    }
}
