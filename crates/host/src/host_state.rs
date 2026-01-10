use alloy::transports::http::reqwest;
use serde::{Deserialize, Serialize};
use tlock_hdk::{tlock_api::entities::EntityId, wasmi_plugin_hdk::plugin_id::PluginId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostState {
    pub plugins: Vec<PluginData>,
    pub entities: Vec<(EntityId, PluginId)>,
    pub state: Vec<((PluginId, String), Vec<u8>)>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PluginData {
    pub id: PluginId,
    pub name: String,
    pub source: PluginSource,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PluginSource {
    Embedded(Vec<u8>),
    Url(String),
}

impl PluginSource {
    pub async fn as_bytes(&self) -> Result<Vec<u8>, reqwest::Error> {
        match self {
            PluginSource::Embedded(bytes) => Ok(bytes.clone()),
            PluginSource::Url(url) => {
                let response = reqwest::get(url).await?;
                let response = response.error_for_status()?;
                let bytes = response.bytes().await?;
                Ok(bytes.to_vec())
            }
        }
    }
}
