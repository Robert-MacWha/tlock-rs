use dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use futures::channel::oneshot;
use gloo_timers::callback::Timeout;
use host::host::Host;
use std::{collections::HashMap, sync::Arc};
use tlock_hdk::{
    tlock_api::entities::EntityId,
    wasmi_hdk::plugin::{PluginError, PluginId},
};

fn main() {
    dioxus::launch(app);
}

#[derive(Clone)]
struct HostContext {
    host: Arc<Host>,
    plugins: Signal<Vec<(PluginId, String)>>,
    entities: Signal<HashMap<EntityId, PluginId>>,
}

impl HostContext {
    async fn load_plugin(
        &mut self,
        wasm_bytes: &[u8],
        name: &str,
    ) -> Result<PluginId, PluginError> {
        let id = self.host.load_plugin(wasm_bytes, name).await?;

        // let mut plugins = self.plugins;
        // plugins.push((id.clone(), name.to_string()));

        // let plugin_entities = self.host.get_entities();
        // self.entities.set(plugin_entities);

        Ok(id)
    }
}

#[component]
fn app() -> Element {
    let host = Arc::new(Host::new());
    let plugins = use_signal(|| Vec::new());
    let entities = use_signal(|| HashMap::new());

    use_context_provider(|| HostContext {
        host,
        plugins,
        entities,
    });

    rsx! {
        document::Stylesheet { href: asset!("/assets/bootstrap.css") }

        div {
            class: "container mx-auto p-4",
            h1 { "Tlock" }
            control_panel {}
            plugin_list {}
            entities_list {}
        }
    }
}

#[component]
fn control_panel() -> Element {
    let state = use_context::<HostContext>();
    let on_wasm_file = move |evt: Event<FormData>| {
        let mut state = state.clone();
        spawn(async move {
            let file_engine = match evt.files() {
                Some(f) => f,
                None => {
                    error!("No file engine");
                    return;
                }
            };

            let files = file_engine.files();
            let file_name = match files.get(0) {
                Some(f) => f,
                None => {
                    error!("No file selected");
                    return;
                }
            };

            info!("Selected file: {}", file_name);
            let file = match file_engine.read_file(file_name).await {
                Some(f) => f,
                None => {
                    error!("Failed to read file");
                    return;
                }
            };

            // let t1 = async {
            //     for i in 0..=10 {
            //         info!("Task 1: {i}");
            //         sleep(100).await;
            //     }
            //     "done 1"
            // };

            // let t2 = async {
            //     for i in 0..=10 {
            //         info!("Task 2: {i}");
            //         sleep(100).await;
            //     }
            //     "done 2"
            // };

            // let (r1, r2) = futures::join!(t1, t2);
            // info!("Tasks finished: {:?}, {:?}", r1, r2);

            match state.load_plugin(&file, file_name).await {
                Ok(id) => {
                    info!("Loaded plugin with id: {}", id);
                }
                Err(e) => {
                    error!("Failed to load plugin: {:?}", e);
                    return;
                }
            };
        });
    };

    rsx! {
        div {
            "Control Panel"
            ul {
                li {
                    input {
                        r#type: "file",
                        accept: ".wasm",
                        onchange: on_wasm_file,
                    }
                }
            }
        }
    }
}

#[component]
fn plugin_list() -> Element {
    let state = use_context::<HostContext>();
    let plugins = state.plugins.read();

    rsx! {
        div {
            "Plugin List:",
            ul {
                for plugin in plugins.iter() {
                    li { key: "{plugin.0}", "{plugin.1} ({plugin.0})" }
                }
            }
        }
    }
}

#[component]
fn entities_list() -> Element {
    let state = use_context::<HostContext>();
    let entities = state.entities.read();

    rsx! {
        div {
            "Entities List:",
            ul {
                for (entity_id, plugin_id) in entities.iter() {
                    li { key: "{entity_id}", "{entity_id} (from plugin {plugin_id})" }
                }
            }
        }
    }
}

async fn sleep(millis: u32) {
    let (send, recv) = oneshot::channel();

    let _timeout = Timeout::new(millis, move || {
        let _ = send.send(());
    });

    let _ = recv.await;
}
