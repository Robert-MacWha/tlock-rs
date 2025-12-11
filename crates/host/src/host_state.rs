use alloy::transports::http::reqwest;
use serde::{Deserialize, Serialize};
use tlock_hdk::{
    tlock_api::{
        component::Component,
        entities::{EntityId, PageId},
    },
    wasmi_plugin_hdk::plugin::PluginId,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostState {
    pub plugins: Vec<PluginData>,
    pub entities: Vec<(EntityId, PluginId)>,
    pub state: Vec<(PluginId, Vec<u8>)>,
    pub interfaces: Vec<(PageId, Component)>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PluginData {
    pub id: PluginId,
    pub name: String,
    pub source: PluginSource,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PluginSource {
    #[serde(with = "base64_serde")]
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

mod base64_serde {
    use base64::{Engine, engine::general_purpose};
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = general_purpose::STANDARD.encode(bytes);
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        general_purpose::STANDARD
            .decode(&s)
            .map_err(serde::de::Error::custom)
    }
}
