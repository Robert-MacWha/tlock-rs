use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Domain {
    Vault,
}

impl Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Domain::Vault => write!(f, "vault"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VaultId(String);

impl VaultId {
    pub const DOMAIN: Domain = Domain::Vault;

    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn domain(&self) -> Domain {
        Self::DOMAIN
    }

    pub fn as_entity_id(&self) -> EntityId {
        EntityId::Vault(self.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityId {
    Vault(VaultId),
}

impl EntityId {
    pub fn domain(&self) -> Domain {
        match self {
            EntityId::Vault(_) => Domain::Vault,
        }
    }
}

impl Display for VaultId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vault:{}", self.0)
    }
}

impl Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntityId::Vault(v) => v.fmt(f),
        }
    }
}

impl From<VaultId> for EntityId {
    fn from(id: VaultId) -> Self {
        EntityId::Vault(id)
    }
}
