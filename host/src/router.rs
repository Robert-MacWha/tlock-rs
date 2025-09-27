use std::{collections::HashMap, sync::Arc};

use log::{error, info, warn};
use thiserror::Error;
use tlock_hdk::{
    async_trait::async_trait,
    tlock_api::{
        CompositeClient,
        alloy_primitives::{Address, Signature, TxHash},
        domains::{
            Domains,
            eip155_keyring::{
                CreateAccountParams, DeleteAccountParams, Eip155Keyring, PersonalSignParams,
                SendTransactionParams, SignTypedDataParams,
            },
            host::HostDomain,
            tlock::TlockDomain,
        },
        routes::{PluginId, Route},
    },
    wasmi_hdk::{plugin::PluginError, wasmi_pdk::rpc_message::RpcErrorCode},
};

/// The router is responsible for managing plugins and routing requests between
/// them. It receives requests from plugins, the frontend, or other sources, and
/// determines which plugin should handle each request. The router also manages
/// the lifecycle of plugins, including loading, unloading, and updating them.
///  
/// The router is also responsible for ensuring that the correct number of plugins
/// are used for various tasks, and for managing fallback mechanisms in case of
/// overlapping functionality.
pub struct Router {
    plugins: HashMap<PluginId, Arc<CompositeClient<PluginError>>>,
    routes: HashMap<(Domains, String), PluginId>,
}

#[derive(Debug, Error)]
pub enum RouterError {
    #[error("Method not found: '{method}'")]
    MethodNotFound { method: String },
    #[error("Cannot resolve plugin for entity: '{entity}'")]
    ResolveError { entity: String },
    #[error("Plugin already registered: '{plugin_id}'")]
    PluginAlreadyRegistered { plugin_id: String },
    #[error("Route already registered for entity: '{entity}' in domain: '{domain:?}'")]
    RouteAlreadyRegistered { entity: String, domain: Domains },
}

impl Router {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            routes: HashMap::new(),
        }
    }

    /// Registers a plugin with the router
    pub fn register_plugin(
        &mut self,
        plugin_id: PluginId,
        client: Arc<CompositeClient<PluginError>>,
    ) -> Result<(), RouterError> {
        if self.plugins.contains_key(&plugin_id) {
            return Err(RouterError::PluginAlreadyRegistered { plugin_id });
        }
        self.plugins.insert(plugin_id, client);
        Ok(())
    }

    /// Registers a route for a specific domain and entity
    pub fn register_route<T: Route>(
        &mut self,
        domain: Domains,
        key: &T,
        plugin_id: PluginId,
    ) -> Result<(), RouterError> {
        let entity_id = key.to_key();

        if self
            .routes
            .contains_key(&(domain.clone(), entity_id.clone()))
        {
            return Err(RouterError::RouteAlreadyRegistered {
                entity: entity_id,
                domain,
            });
        }
        self.routes.insert((domain, entity_id), plugin_id);
        Ok(())
    }

    /// Resolves the appropriate plugin for a given domain and entity key
    fn resolve_plugin<T: Route>(
        &self,
        domain: &Domains,
        key: &T,
    ) -> Result<Arc<CompositeClient<PluginError>>, RpcErrorCode> {
        let entity_id = key.to_key();
        let plugin_id = self
            .routes
            .get(&(domain.clone(), entity_id.clone()))
            .ok_or_else(|| {
                // May happen if a invalid request is made
                warn!("Cannot resolve route for entity: {}", entity_id);
                RpcErrorCode::MethodNotFound
            })?;

        let plugin = self.plugins.get(plugin_id).ok_or_else(|| {
            // Should never happen if the router is working
            error!("Cannot resolve plugin from plugin_id: {}", plugin_id);
            RpcErrorCode::MethodNotFound
        })?;

        Ok(plugin.clone())
    }
}

#[async_trait]
impl TlockDomain for Router {
    type Error = PluginError;

    async fn name(&self) -> Result<String, Self::Error> {
        Ok("router".to_string())
    }

    async fn version(&self) -> Result<String, Self::Error> {
        Ok("0.1.0".to_string())
    }

    async fn ping(&self, message: String) -> Result<String, Self::Error> {
        Ok(format!("Router Pong: {}", message))
    }
}

#[async_trait]
impl Eip155Keyring for Router {
    type Error = PluginError;

    async fn create_account(&self, params: CreateAccountParams) -> Result<Address, Self::Error> {
        let plugin = self.resolve_plugin(&Domains::Eip155Keyring, &params.route)?;
        plugin.eip155_keyring().create_account(params).await
    }

    async fn delete_account(&self, params: DeleteAccountParams) -> Result<(), Self::Error> {
        let plugin = self.resolve_plugin(&Domains::Eip155Keyring, &params.route)?;
        plugin.eip155_keyring().delete_account(params).await
    }

    async fn personal_sign(&self, params: PersonalSignParams) -> Result<Signature, Self::Error> {
        let plugin = self.resolve_plugin(&Domains::Eip155Keyring, &params.route)?;
        plugin.eip155_keyring().personal_sign(params).await
    }

    async fn sign_typed_data(&self, params: SignTypedDataParams) -> Result<Signature, Self::Error> {
        let plugin = self.resolve_plugin(&Domains::Eip155Keyring, &params.route)?;
        plugin.eip155_keyring().sign_typed_data(params).await
    }

    async fn send_transaction(&self, params: SendTransactionParams) -> Result<TxHash, Self::Error> {
        let plugin = self.resolve_plugin(&Domains::Eip155Keyring, &params.route)?;
        plugin.eip155_keyring().send_transaction(params).await
    }
}

#[async_trait]
impl HostDomain for Router {
    type Error = PluginError;

    async fn register_entity(&self, domain: Domains, key: String) -> Result<(), Self::Error> {
        // TODO: Have a permission system here to restrict who can register entities
        info!("Registering entity: {} in domain: {:?}", key, domain);

        Ok(())
    }
}
