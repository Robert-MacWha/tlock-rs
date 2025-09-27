use alloy_primitives::{Address, ChainId};
use serde::{Deserialize, Serialize};

pub type PluginId = String;

/// A trait for types that can be used as routes to uniquely identify entities.
pub trait Route {
    fn to_key(&self) -> String;
}

#[derive(Debug, Serialize, Deserialize)]
/// A route that identifies an entity by its ID.
pub struct PluginIdRoute {
    pub plugin_id: PluginId,
}
impl Route for PluginIdRoute {
    fn to_key(&self) -> String {
        format!("plugin:{}", self.plugin_id)
    }
}

/// A route that identifies an entity by an eip155 chain ID.
#[derive(Debug, Serialize, Deserialize)]
pub struct Eip155ChainRoute {
    pub chain_id: ChainId,
}
impl Route for Eip155ChainRoute {
    fn to_key(&self) -> String {
        format!("eip155Chain:{}", self.chain_id)
    }
}

/// A route that identifies an entity by its eip155 chain ID and address.
#[derive(Debug, Serialize, Deserialize)]
pub struct Eip155AccountRoute {
    pub chain_id: ChainId,
    pub account: Address,
}
impl Route for Eip155AccountRoute {
    fn to_key(&self) -> String {
        format!("eip155Account:{}", self.account)
    }
}
