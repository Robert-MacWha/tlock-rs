use std::{collections::HashMap, sync::Arc};

use dioxus::{
    hooks::{UnboundedReceiver, use_coroutine, use_signal},
    signals::{ReadableExt, Signal, WritableExt},
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
    pub host: Signal<Arc<Host>>,
    pub plugins: Signal<Vec<PluginId>>,
    pub entities: Signal<Vec<EntityId>>,
    pub interfaces: Signal<HashMap<PageId, Component>>,
    pub user_requests: Signal<Vec<UserRequest>>,
    pub event_log: Signal<Vec<String>>,
}

impl HostContext {
    pub fn new(host: Arc<Host>) -> Self {
        let ctx = Self {
            host: use_signal(|| host),
            plugins: use_signal(Vec::new),
            entities: use_signal(Vec::new),
            interfaces: use_signal(HashMap::new),
            user_requests: use_signal(Vec::new),
            event_log: use_signal(Vec::new),
        };
        ctx.setup_subscription();
        ctx
    }

    pub fn set_host(&mut self, host: Arc<Host>) {
        self.host.set(host);
        self.setup_subscription();
        self.host.write().notify_observers();
    }

    fn setup_subscription(&self) {
        let host = self.host.clone();
        let mut plugins_sig = self.plugins;
        let mut entities_sig = self.entities;
        let mut interfaces_sig = self.interfaces;
        let mut user_requests_sig = self.user_requests;
        let mut event_log_sig = self.event_log;

        let coro = use_coroutine(move |mut rx: UnboundedReceiver<()>| {
            let host = host.clone();
            async move {
                while let Some(()) = rx.next().await {
                    plugins_sig.set(host.read().get_plugins());
                    entities_sig.set(host.read().get_entities());
                    interfaces_sig.set(host.read().get_interfaces());
                    user_requests_sig.set(host.read().get_user_requests());
                    event_log_sig.set(host.read().get_event_log());
                }
            }
        });

        self.host.read().subscribe(coro.tx());
    }
}
