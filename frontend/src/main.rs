use std::sync::Arc;

use dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use frontend::{
    components::{entity::Entity, page::Page, user_requests::UserRequestComponent},
    contexts::host::HostContext,
    download_util::trigger_file_download,
};
use host::{
    host::Host,
    host_state::{HostState, PluginSource},
};
use tlock_hdk::tlock_api::entities::{EntityId, PageId};

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
            div {
                class: "row",
                div {
                    class: "col-lg-8",
                    control_panel {}
                    request_list {}
                    page_list {}
                }
                div {
                    class: "col-lg-4",
                    plugin_list {}
                    entities_list {}
                    event_log {}
                }
            }
        }
    }
}

#[component]
fn control_panel() -> Element {
    let on_wasm_file = move |e: Event<FormData>| {
        spawn(async move {
            e.prevent_default();

            let files = e.files();
            let Some(file) = files.first() else {
                error!("No file selected");
                return;
            };

            let name = file.name();
            let Ok(data) = file.read_bytes().await else {
                error!("Failed to read file: {}", name);
                return;
            };

            let plugin_source = PluginSource::Embedded(data.to_vec());
            match consume_context::<HostContext>()
                .host
                .read()
                .new_plugin(plugin_source, name.strip_suffix(".wasm").unwrap_or(&name))
                .await
            {
                Ok(id) => {
                    info!("Loaded plugin with id: {}", id);
                }
                Err(e) => {
                    error!("Failed to load plugin: {:?}", e);
                }
            }
        });
    };

    let on_host_state = {
        move |e: Event<FormData>| {
            spawn(async move {
                e.prevent_default();

                let files = e.files();
                let Some(file) = files.first() else {
                    error!("No file selected");
                    return;
                };

                let name = file.name();
                let Ok(data) = file.read_bytes().await else {
                    error!("Failed to read file: {}", name);
                    return;
                };

                let host_state: HostState = match serde_json::from_slice(&data) {
                    Ok(state) => state,
                    Err(e) => {
                        error!("Failed to parse host state JSON: {:?}", e);
                        return;
                    }
                };

                let host = match Host::from_state(host_state).await {
                    Ok(host) => host,
                    Err(e) => {
                        error!("Failed to load host state: {:?}", e);
                        return;
                    }
                };

                consume_context::<HostContext>().set_host(host);
            });
        }
    };

    let save_host_state = {
        move |_: Event<MouseData>| {
            spawn(async move {
                let host_state = consume_context::<HostContext>().host.read().to_state();
                let json = match serde_json::to_string_pretty(&host_state) {
                    Ok(json) => json,
                    Err(e) => {
                        error!("Failed to serialize host state: {:?}", e);
                        return;
                    }
                };

                match trigger_file_download(
                    "host_state.json",
                    "application/json",
                    json.into_bytes(),
                ) {
                    Ok(_) => info!("Host state download triggered"),
                    Err(e) => error!("Failed to trigger host state download: {:?}", e),
                }
            });
        }
    };

    rsx! {
        div {
            h5 { "Control Panel" }
            ul {
                li {
                    button {
                        onclick: save_host_state,
                        "Save Host State"
                    }
                }
                li {
                    label {for: "host_state_input", "Load Host State"}
                    br {  }
                    input {
                        r#type: "file",
                        accept: "application/json",
                        onchange: on_host_state,
                        name: "host_state_input",
                    }
                }
                li {
                    label {for: "wasm_file_input", "Load WASM Plugin"}
                    br {  }
                    input {
                        r#type: "file",
                        accept: ".wasm",
                        onchange: on_wasm_file,
                        name: "wasm_file_input",
                    }
                }
            }
        }
    }
}

#[component]
fn request_list() -> Element {
    let state: HostContext = use_context();
    let requests = state.user_requests.read();

    rsx! {
        div {
            h5 { "User Requests:" },
            if requests.is_empty() {
                div { class: "text-muted", "No pending requests" }
            } else {
                ul {
                    for (index, request) in requests.iter().enumerate() {
                        li {
                            key: "{index}",
                            UserRequestComponent { request: request.clone() }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn page_list() -> Element {
    let state = use_context::<HostContext>();
    let entities = state.entities.read();
    let pages: Vec<PageId> = entities
        .iter()
        .filter_map(|e| match e {
            EntityId::Page(id) => Some(*id),
            _ => None,
        })
        .collect();

    rsx! {
        div {
            h5 { "Pages:" },
            ul {
                for page_id in pages {
                    li {
                        key: "{page_id}",
                        Page { id: page_id }
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
    let named_plugins = plugins
        .iter()
        .filter_map(|id| state.host.read().get_plugin(id));

    rsx! {
        div {
            h5 { "Plugin List:" },
            ul {
                for plugin in named_plugins {
                    li { key: "{plugin.id()}", "{plugin.name()} (id = {plugin.id()})" }
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
            h5 { "Entities List:" },
            ul {
                for entity_id in entities.iter() {
                    li {
                        key: "{entity_id}",
                        Entity { id: *entity_id }
                    }
                }
            }
        }
    }
}

#[component]
fn event_log() -> Element {
    let state = use_context::<HostContext>();
    let log = state.event_log.read();

    rsx! {
        div {
            h5 { "Event Log:" },
            ul {
                for (index, event) in log.iter().enumerate() {
                    li { key: "{index}", "{event}" }
                }
            }
        }
    }
}
