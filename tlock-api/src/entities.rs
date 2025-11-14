use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct VaultId(Uuid);
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PageId(Uuid);

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct EthProviderId(Uuid);

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum EntityId {
    Vault(VaultId),
    Page(PageId),
    EthProvider(EthProviderId),
}

impl Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntityId::Vault(vault_id) => write!(f, "{}", vault_id),
            EntityId::Page(page_id) => write!(f, "{}", page_id),
            EntityId::EthProvider(eth_provider_id) => write!(f, "{}", eth_provider_id),
        }
    }
}

impl VaultId {
    pub fn new() -> Self {
        VaultId(Uuid::new_v4())
    }
}

impl Display for VaultId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vault:{}", self.0)
    }
}

impl From<VaultId> for EntityId {
    fn from(vault_id: VaultId) -> Self {
        EntityId::Vault(vault_id)
    }
}

impl PageId {
    pub fn new() -> Self {
        PageId(Uuid::new_v4())
    }
}

impl Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "page:{}", self.0)
    }
}

impl From<PageId> for EntityId {
    fn from(page_id: PageId) -> Self {
        EntityId::Page(page_id)
    }
}

impl EthProviderId {
    pub fn new() -> Self {
        EthProviderId(Uuid::new_v4())
    }
}

impl Display for EthProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "eth_provider:{}", self.0)
    }
}

impl From<EthProviderId> for EntityId {
    fn from(eth_provider_id: EthProviderId) -> Self {
        EntityId::EthProvider(eth_provider_id)
    }
}
