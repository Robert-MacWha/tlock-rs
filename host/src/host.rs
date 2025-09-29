use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use alloy_primitives::U256;
use log::info;
use tlock_hdk::{
    dispatcher::RpcHandler,
    tlock_api::{
        caip::{AccountId, AssetId},
        entities::{EntityId, ToEntityId, VaultId},
        global, host, vault,
    },
    wasmi_hdk::plugin::{Plugin, PluginId},
    wasmi_pdk::{async_trait::async_trait, rpc_message::RpcErrorCode},
};

pub struct Host {
    plugins: Mutex<HashMap<PluginId, Arc<Plugin>>>,
}

impl Host {
    pub fn new() -> Self {
        Self {
            plugins: Mutex::new(HashMap::new()),
        }
    }

    pub fn register_plugin(&self, plugin: Arc<Plugin>) {
        let mut plugins = self.plugins.lock().unwrap();
        plugins.insert(plugin.id(), plugin);
    }
}

#[async_trait]
impl RpcHandler<global::Ping> for Host {
    async fn invoke(&self, plugin_id: PluginId, _params: ()) -> Result<String, RpcErrorCode> {
        info!("Plugin {} sent ping", plugin_id);
        Ok(format!("Pong from host"))
    }
}

#[async_trait]
impl RpcHandler<host::CreateEntity> for Host {
    async fn invoke(&self, plugin_id: PluginId, params: EntityId) -> Result<(), RpcErrorCode> {
        info!(
            "Plugin {} requested creation of entity {:?}",
            plugin_id, params
        );

        Err(RpcErrorCode::MethodNotFound)
    }
}

#[async_trait]
impl RpcHandler<vault::BalanceOf> for Host {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        params: VaultId,
    ) -> Result<Vec<(AssetId, U256)>, RpcErrorCode> {
        info!("Plugin {} requested balance of {:?}", plugin_id, params);

        Err(RpcErrorCode::MethodNotFound)
    }
}

#[async_trait]
impl RpcHandler<vault::Transfer> for Host {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        params: (VaultId, AccountId, AssetId, U256),
    ) -> Result<Result<(), String>, RpcErrorCode> {
        let (vault, to, asset, amount) = params;
        info!(
            "Plugin {} requested transfer of {} {:?} from {:?} to {:?}",
            plugin_id,
            amount,
            asset,
            vault.to_id(),
            to
        );

        Err(RpcErrorCode::MethodNotFound)
    }
}

#[async_trait]
impl RpcHandler<vault::GetReceiptAddress> for Host {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        params: (VaultId, AssetId),
    ) -> Result<Result<AccountId, String>, RpcErrorCode> {
        info!(
            "Plugin {} requested receipt address for {:?}",
            plugin_id, params
        );

        Err(RpcErrorCode::MethodNotFound)
    }
}

#[async_trait]
impl RpcHandler<vault::OnReceive> for Host {
    async fn invoke(
        &self,
        plugin_id: PluginId,
        params: (VaultId, AssetId),
    ) -> Result<(), RpcErrorCode> {
        info!(
            "Plugin {} notified of received asset {:?}",
            plugin_id, params
        );

        Err(RpcErrorCode::MethodNotFound)
    }
}
