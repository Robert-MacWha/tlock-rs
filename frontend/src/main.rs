use dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use frontend::{components::entity::Entity, contexts::host::HostContext};
use host::host::Host;
use std::sync::Arc;
fn main() {
    dioxus::launch(app);
}

#[component]
fn app() -> Element {
    let host = Arc::new(Host::new());
    let host_context = HostContext::new(host.clone());
    use_context_provider(|| host_context);

    spawn(async {
        loop {
            gloo_timers::future::TimeoutFuture::new(1000).await;
            info!("heartbeat");
        }
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
            let Some(file_engine) = evt.files() else {
                error!("No file engine available");
                return;
            };

            let files = file_engine.files();
            let Some(file_name) = files.get(0) else {
                error!("No file selected");
                return;
            };

            info!("Selected file: {}", file_name);
            let Some(file) = file_engine.read_file(file_name).await else {
                error!("Failed to read file: {}", file_name);
                return;
            };

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
    let named_plugins = plugins.iter().filter_map(|id| state.host.get_plugin(id));

    rsx! {
        div {
            "Plugin List:",
            ul {
                for plugin in named_plugins {
                    li { key: "{plugin.id()}", "{plugin.name()} ({plugin.id()})" }
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
                for entity_id in entities.iter() {
                    li {
                        key: "{entity_id}",
                        Entity { id: entity_id.clone() }
                     }
                }
            }
        }
    }
}
