use std::{collections::HashMap, sync::Arc};

use dioxus::{
    hooks::{UnboundedReceiver, use_coroutine, use_signal},
    signals::{Signal, WritableExt},
};
use futures::StreamExt;
use host::host::{Host, UserRequest};
use tlock_hdk::{
    tlock_api::{
        component::Component,
        entities::{EntityId, PageId},
    },
    wasmi_plugin_hdk::plugin::PluginId,
};

#[derive(Clone)]
pub struct HostContext {
    pub host: Arc<Host>,
    pub plugins: Signal<Vec<PluginId>>,
    pub entities: Signal<Vec<EntityId>>,
    pub interfaces: Signal<HashMap<PageId, Component>>,
    pub user_requests: Signal<Vec<UserRequest>>,
    pub event_log: Signal<Vec<String>>,
}

impl HostContext {
    pub fn new(host: Arc<Host>) -> Self {
        let plugins = use_signal(Vec::new);
        let entities = use_signal(Vec::new);
        let interfaces = use_signal(HashMap::new);
        let user_requests = use_signal(Vec::new);
        let event_log = use_signal(Vec::new);

        let host_clone = host.clone();
        let coro = use_coroutine(move |mut rx: UnboundedReceiver<()>| {
            let host = host_clone.clone();
            let mut plugins_sig = plugins;
            let mut entities_sig = entities;
            let mut interfaces_sig = interfaces;
            let mut user_requests_sig = user_requests;
            let mut event_log_sig = event_log;

            async move {
                while let Some(()) = rx.next().await {
                    plugins_sig.set(host.get_plugins());
                    entities_sig.set(host.get_entities());
                    interfaces_sig.set(host.get_interfaces());
                    user_requests_sig.set(host.get_user_requests());
                    event_log_sig.set(host.get_event_log());
                }
            }
        });

        host.subscribe(coro.tx());

        Self {
            host,
            plugins,
            entities,
            interfaces,
            user_requests,
            event_log,
        }
    }
}
