use dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use frontend::{components::entity::Entity, contexts::host::HostContext};
use host::host::Host;
use std::sync::Arc;
use tlock_hdk::{tlock_api, wasmi_hdk::plugin::PluginId};
fn main() {
    console_error_panic_hook::set_once();
    dioxus::launch(app);
}

#[component]
fn app() -> Element {
    let host = Arc::new(Host::new());
    let host_context = HostContext::new(host.clone());
    use_context_provider(|| host_context);

    let block_number = use_resource({
        let host = host.clone();
        move || {
            let host = host.clone();
            async move {
                // let client = reqwest::Client::new();
                // let req = client
                //     .post("https://eth.llamarpc.com")
                //     .json(&serde_json::json!({
                //         "jsonrpc": "2.0",
                //         "method": "eth_blockNumber",
                //         "params": [],
                //         "id": 1,
                //     }));

                // info!("Sending block number request: {:?}", req);
                // let resp = req.send().await.unwrap();
                // info!("Received block number response: {:?}", resp);
                // let resp_json: serde_json::Value = resp.json().await.unwrap();
                // info!("Fetched block number response: {:?}", resp_json);

                let resp = host
                    .fetch(
                        &"test plugin".into(),
                        tlock_api::host::Request {
                            url: "https://eth.llamarpc.com".to_string(),
                            method: "POST".to_string(),
                            headers: vec![(
                                "Content-Type".to_string(),
                                "application/json".as_bytes().into(),
                            )],
                            body: Some(
                                serde_json::to_vec(&serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "method": "eth_blockNumber",
                                    "params": [],
                                    "id": 1,
                                }))
                                .unwrap(),
                            ),
                        },
                    )
                    .await;

                info!("Fetched block number response: {:?}", resp);
            }
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
