use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

/// Domains represent the different possible semantic categories of entities.
/// All entities from a given domain must share a common interface, but may
/// have different internal implementations and behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Domain {
    /// Vaults can hold, transfer, and receive assets.
    Vault,
    /// Pages can render themselves in a web UI as a whole page.
    Page,
    /// EthProviders can provide Ethereum-style RPC access.
    EthProvider,
}

impl Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Domain::Vault => write!(f, "vault"),
            Domain::Page => write!(f, "page"),
            Domain::EthProvider => write!(f, "ethprovider"),
        }
    }
}
