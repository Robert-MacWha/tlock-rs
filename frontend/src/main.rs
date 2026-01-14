use std::sync::Arc;

use anyhow::anyhow;
use dioxus::{logger::tracing::info, prelude::*};
use frontend::{
    components::{page::Page, user_requests::UserRequestComponent},
    contexts::{
        host::HostContext,
        toast::{ToastContext, ToastKind, toast_container},
    },
    download_util::download_bytes,
    focus_helper::blur_active_element,
};
use host::{host::Host, host_state::PluginSource};
use tlock_hdk::tlock_api::{
    entities::{EntityId, PageId},
    host::NotifyLevel,
};

#[derive(Copy, Clone)]
struct UiContext {
    show_request_sidebar: Signal<bool>,
    show_events_sidebar: Signal<bool>,
    show_plugin_registry_sidebar: Signal<bool>,
    selected_page: Signal<Option<PageId>>,
}

fn main() {
    console_error_panic_hook::set_once();

    dioxus::launch(app);
}

#[component]
fn app() -> Element {
    let host = Arc::new(Host::new());
    let host_context = HostContext::new(host.clone());
    use_context_provider(|| host_context);

    let ui_signals = UiContext {
        show_request_sidebar: use_signal(|| false),
        show_events_sidebar: use_signal(|| false),
        show_plugin_registry_sidebar: use_signal(|| false),
        selected_page: use_signal(|| None),
    };
    use_context_provider(|| ui_signals);

    let toasts = use_signal(Vec::new);
    use_context_provider(|| ToastContext::new(toasts));

    rsx! {
        document::Stylesheet { href: asset!("/assets/tailwind.css") }
        //? DaisyUI theme
        div { "data-theme": "light",
            toast_container {}
            requests_modal {}
            events_modal {}
            plugins_modal {}
            events_toast_handler {}
            div { class: "drawer md:drawer-open",
                input {
                    id: "my-drawer",
                    r#type: "checkbox",
                    class: "drawer-toggle",
                }
                div { class: "drawer-content flex flex-col",
                    label {
                        r#for: "my-drawer",
                        class: "btn btn-square btn-ghost md:hidden",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            fill: "none",
                            view_box: "0 0 24 24",
                            class: "inline-block w-6 h-6 stroke-current",
                            path {
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                stroke_width: 2,
                                d: "M4 6h16M4 12h16M4 18h16",
                            }
                        }
                    }
                    div { class: "w-full min-h-full p-4 bg-base-300", main_component {} }
                }
                div { class: "drawer-side",
                    label {
                        r#for: "my-drawer",
                        aria_label: "close drawer",
                        class: "drawer-overlay",
                    }
                    sidebar_component {}
                }
            }
        }
    }
}

#[component]
fn sidebar_component() -> Element {
    let ctx: HostContext = use_context();
    let mut show_requests = use_context::<UiContext>().show_request_sidebar;
    let mut show_events = use_context::<UiContext>().show_events_sidebar;
    let mut selected_page = use_context::<UiContext>().selected_page;
    let mut show_plugin_registry = use_context::<UiContext>().show_plugin_registry_sidebar;

    let named_pages = use_memo(move || {
        let pages = ctx.page_ids();
        let named_pages: Vec<_> = pages
            .into_iter()
            .map(|id| {
                let name = ctx
                    .entity_plugin(EntityId::Page(id))
                    .map(|p| p.name().to_string())
                    .unwrap_or("Unknown Plugin".to_string());

                (id, name)
            })
            .collect();

        named_pages
    });

    let named_entities = use_memo(move || {
        let entities = ctx.entity_ids();
        let entities = entities
            .into_iter()
            .filter(|id| !matches!(id, EntityId::Page(_)));

        let named_entities: Vec<_> = entities
            .map(|id| {
                let name = ctx
                    .entity_plugin(id)
                    .map(|p| p.name().to_string())
                    .unwrap_or("Unknown Plugin".to_string());

                (id, name)
            })
            .collect();

        named_entities
    });

    rsx! {
        div { class: "flex flex-col h-full bg-base-200 w-xs menu",
            h1 { class: "menu-title text-xl text-primary", "Lodgelock" }
            states_dropdown {}
            div { class: "divider" }
            h2 { class: "menu-title", "Pages" }
            ul {
                li { key: "home",
                    button {
                        class: if selected_page.read().is_none() { "menu-active" },
                        class: "py-1.5",
                        onclick: move |_| selected_page.set(None),
                        "Home"
                    }
                }
                for (page_id , plugin_name) in named_pages() {
                    li { key: "page-{page_id}",
                        button {
                            class: "py-1.5 tooltip",
                            class: if selected_page.read().as_ref() == Some(&page_id) { "menu-active" },
                            "data-tip": "plugin: {plugin_name}",
                            onclick: move |_| selected_page.set(Some(page_id)),
                            "{page_id}"
                        }
                    }
                }
            }
            h2 { class: "menu-title", "Entities" }
            ul { class: "px-3",
                for (entity_id , plugin_name) in named_entities() {
                    p {
                        key: "entity-{entity_id}",
                        class: "py-1.5 w-full tooltip",
                        "data-tip": "plugin: {plugin_name}",
                        "{entity_id}"
                    }
                }
            }
            h2 { class: "menu-title", "Plugins" }
            ul { class: "px-3",
                for plugin in ctx.plugins() {
                    p { key: "plugin-{plugin.id()}", class: "py-1.5 w-full",
                        "{plugin.name()} [{plugin.id()}]"
                    }
                }
            }
            div { class: "grow" }
            div { class: "divider" }
            ul {
                li {
                    button {
                        class: "indicator w-full justify-between",
                        onclick: move |_| show_requests.set(true),
                        "Requests"

                        if !ctx.requests().is_empty() {
                            span { class: "indicator-item badge badge-primary mr-4",
                                "{ctx.requests().len()}"
                            }
                        }
                    }
                }
                li {
                    button { onclick: move |_| show_events.set(true), "Events" }
                }
                li {
                    button { onclick: move |_| show_plugin_registry.set(true), "Load Plugin" }
                }
                li {
                    button {
                        onclick: move |_| {
                            let state = ctx.state();
                            if let Ok(data) = serde_json::to_vec_pretty(&state) {
                                if let Err(e) = download_bytes(&data, "state.json", "application/json") {
                                    error!("Failed to download state: {:?}", e);
                                }
                            }
                        },
                        "Download State"
                    }
                }
                        // fieldset { class: "px-3 py-2",
            //     legend { "Upload State file" }
            //     input {
            //         class: "file-input file-input-sm px-0 py-0",
            //         r#type: "file",
            //         accept: ".json",
            //         onchange: move |e| async move {
            //             let toast_ctx: ToastContext = use_context();
            //             if let Err(e) = handle_upload_state(e).await {
            //                 error!("State upload failed: {:?}", e);
            //                 toast_ctx.push(format!("State upload failed: {:?}", e), ToastKind::Error);
            //             } else {
            //                 toast_ctx.push("State uploaded", ToastKind::Info);
            //             }
            //         },
            //     }
            // }
            }
        }
    }
}

async fn handle_upload_state(event: Event<FormData>) -> anyhow::Result<()> {
    let files = event.files();
    let file = files.get(0).ok_or_else(|| anyhow!("No file selected"))?;
    let bytes = file
        .read_bytes()
        .await
        .map_err(|e| anyhow!("Failed to read file bytes: {:?}", e))?;

    let state: host::host_state::HostState = serde_json::from_slice(&bytes)?;
    let host = Host::from_state(state)
        .await
        .map_err(|e| anyhow!("Failed to create host from state: {:?}", e))?;

    let mut ctx: HostContext = consume_context();
    ctx.set_host(host);

    Ok(())
}

#[component]
fn main_component() -> Element {
    let ctx: HostContext = use_context();
    let selected_page = use_context::<UiContext>().selected_page;
    let pages = ctx.page_ids();

    //? If a page is selected, only show that page
    let pages = match selected_page.read().as_ref() {
        Some(page_id) => vec![*page_id],
        None => pages,
    };

    rsx! {
        if pages.is_empty() {
            p { "No pages available. Load a plugin to get started." }
        }

        div { class: if selected_page.read().is_none() { "columns-1 ml:columns-2 2xl:columns-3 gap-4 space-y-4" },
            for page_id in pages {
                {
                    let plugin = ctx.entity_plugin(EntityId::Page(page_id));
                    let plugin_name = plugin
                        .as_ref()
                        .map(|p| p.name().to_string())
                        .unwrap_or("Unknown Plugin".to_string());
                    rsx! {
                        div {
                            key: "{page_id}",
                            class: "card bg-base-200 shadow-sm relative break-inside-avoid mb-4",
                            div { class: "card-body",
                                div { class: "absolute top-4 right-4",
                                    div { class: "badge badge-primary", "{plugin_name} [{page_id}]" }
                                }
                                Page { id: page_id }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn requests_modal() -> Element {
    let ctx: HostContext = use_context();
    let mut show_requests = use_context::<UiContext>().show_request_sidebar;

    use_effect(move || {
        if ctx.requests().is_empty() {
            show_requests.set(false);
        }
    });
    let modal_class = if *show_requests.read() {
        "modal-open"
    } else {
        ""
    };

    rsx! {
        dialog { class: "modal modal-start {modal_class}",
            div { class: "modal-box bg-base-200 w-md",
                h3 { class: "font-bold text-lg", "User Requests" }
                div { class: "divider" }
                if ctx.requests().is_empty() {
                    p { "No pending requests" }
                }

                div { class: "flex flex-col gap-4",
                    for request in ctx.requests() {
                        div { key: "request-{request.id()}",
                            div { class: "card bg-base-100 shadow-sm",
                                div { class: "card-body",
                                    UserRequestComponent { request }
                                }
                            }
                        }
                    }
                }
            }
            form {
                method: "dialog",
                class: "modal-backdrop",
                onmousedown: move |_| show_requests.set(false),
                button { "Close" }
            }
        }
    }
}

#[component]
fn events_modal() -> Element {
    let ctx: HostContext = use_context();
    let mut show_events = use_context::<UiContext>().show_events_sidebar;

    let modal_class = if *show_events.read() {
        "modal-open"
    } else {
        ""
    };
    rsx! {
        dialog { class: "modal modal-start {modal_class}",
            div { class: "modal-box bg-base-200 w-md flex flex-col h-full",
                div { class: "flex-none",
                    h3 { class: "font-bold text-lg", "Events" }
                    div { class: "divider" }
                }
                if ctx.events().is_empty() {
                    p { "Load a plugin to see events" }
                }

                ul { class: "flex-1 overflow-y-auto min-h-0",
                    for event in ctx.events() {
                        {
                            let ts = event.timestamp.format("%H:%M:%S%.3f");
                            rsx! {
                                li {
                                    key: "{event.id}",
                                    class: "flex items-baseline gap-3 py-0.5 px-2 hover:bg-base-300 rounded transition-colors",
                                    span { class: "font-mono text-[10px] uppercase opacity-80 shrink-0", "{ts}" }
                                    span { class: "text-sm break-all", "{event.message}" }
                                }
                            }
                        }
                    }
                }
            }
            form {
                method: "dialog",
                class: "modal-backdrop",
                onmousedown: move |_| show_events.set(false),
                button { "Close" }
            }
        }
    }
}

#[component]
fn plugins_modal() -> Element {
    let ctx: HostContext = use_context();
    let mut show_plugins = use_context::<UiContext>().show_plugin_registry_sidebar;
    let toast_ctx: ToastContext = use_context();

    let loaded_plugins = ctx.plugins();

    let plugins_folder = asset!("/assets/plugins");
    let manifest = use_resource(move || async move {
        let manifest_path = format!("{}/manifest.json", plugins_folder);
        let manifest = dioxus::asset_resolver::read_asset_bytes(&manifest_path)
            .await
            .unwrap();
        let manifest: Vec<String> = serde_json::from_slice(&manifest).unwrap();
        manifest
    });

    let modal_class = if *show_plugins.read() {
        "modal-open"
    } else {
        ""
    };

    // Filter out already loaded plugins based on their names
    let plugins = manifest.read();
    let plugins = plugins.as_ref().map(|m| {
        m.iter()
            .cloned()
            .filter(|name| !loaded_plugins.iter().any(|p| p.name() == name.as_str()))
            .collect::<Vec<_>>()
    });

    rsx!(
        dialog { class: "modal modal-start {modal_class}",
            div { class: "modal-box bg-base-200 w-md flex flex-col h-full",
                div { class: "flex-none w-full menu",
                    h3 { class: "font-bold text-lg", "Plugins" }
                    div { class: "divider" }

                    ul { class: "flex-1 overflow-y-auto min-h-0",
                        if let Some(plugins) = plugins {
                            for plugin_name in plugins.iter() {
                                {
                                    let plugin_name = plugin_name.clone();
                                    rsx! {
                                        li { key: "plugin-{plugin_name}",
                                            button {
                                                class: "text-sm break-all",
                                                onclick: move |_| {
                                                    let plugin_name = plugin_name.clone();
                                                    async move {
                                                        let plugin_path = format!("{}/{}.wasm", plugins_folder, plugin_name);
                                                        show_plugins.set(false);
                                                        if let Err(e) = handle_load_plugin(plugin_path).await {
                                                            error!("Failed to load plugin {}: {:?}", plugin_name, e);
                                                            toast_ctx
                                                                .push(
                                                                    format!("Failed to load plugin {}: {:?}", plugin_name, e),
                                                                    ToastKind::Error,
                                                                );
                                                        } else {
                                                            info!("Successfully loaded plugin {}", plugin_name);
                                                            toast_ctx
                                                                .push(format!("Loaded plugin {}", plugin_name), ToastKind::Info);
                                                        }
                                                    }
                                                },
                                                "{plugin_name}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            form {
                method: "dialog",
                class: "modal-backdrop",
                onmousedown: move |_| show_plugins.set(false),
                button { "Close" }
            }
        }
    )
}

#[component]
fn states_dropdown() -> Element {
    let states_folder = asset!("/assets/states");
    let manifest = use_resource(move || async move {
        let manifest_path = format!("{}/manifest.json", states_folder);
        let manifest = dioxus::asset_resolver::read_asset_bytes(&manifest_path)
            .await
            .unwrap();
        let manifest: Vec<String> = serde_json::from_slice(&manifest).unwrap();
        manifest
    });

    rsx! {
        if let Some(states) = manifest.read().as_ref() {
            div { class: "dropdown",
                div {
                    tabindex: "0",
                    role: "button",
                    class: "btn btn-primary w-full m-1",
                    "Load Demo"
                }
                ul {
                    tabindex: "-1",
                    class: "dropdown-content menu w-full bg-base-100 rounded-box z-1 p-2 shadow-sm",
                    for state_name in states.iter() {
                        {
                            let state_name = state_name.clone();
                            rsx! {
                                li { key: "state-{state_name}",
                                    button {
                                        class: "text-sm break-all",
                                        onclick: move |_| {
                                            let state_name = state_name.clone();
                                            async move {
                                                blur_active_element();


                                                let state_path = format!("{}/{}.json", states_folder, state_name);
                                                if let Err(e) = handle_load_state(&state_path).await {
                                                    error!("Failed to load state {}: {:?}", state_name, e);
                                                } else {
                                                    info!("Successfully loaded state {}", state_name);
                                                }
                                            }
                                        },
                                        "{state_name}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            p { "Loading states..." }
        }
    }
}

async fn handle_load_state(path: &str) -> anyhow::Result<()> {
    info!("Loading state from path: {}", path);
    let state_bytes = dioxus::asset_resolver::read_asset_bytes(path)
        .await
        .map_err(|e| anyhow!("Failed to read state asset bytes: {:?}", e))?;

    let state: host::host_state::HostState = serde_json::from_slice(&state_bytes)
        .map_err(|e| anyhow!("Failed to deserialize state JSON: {:?}", e))?;
    let host = Host::from_state(state)
        .await
        .map_err(|e| anyhow!("Failed to create host from state: {:?}", e))?;

    let mut ctx: HostContext = consume_context();
    ctx.set_host(host);

    Ok(())
}

async fn handle_load_plugin(path: String) -> anyhow::Result<()> {
    info!("Loading plugin from path: {}", path);

    // Get the current origin and make the URL absolute
    let window = web_sys::window().ok_or_else(|| anyhow::anyhow!("No window"))?;
    let location = window.location();
    let origin = location
        .origin()
        .map_err(|_| anyhow::anyhow!("Failed to get origin"))?;
    let full_url = format!("{}{}", origin, path);

    info!("Full URL: {}", full_url);
    let plugin_source = PluginSource::Url(full_url);
    let name = path
        .split('/')
        .last()
        .and_then(|s| s.strip_suffix(".wasm"))
        .unwrap_or("unknown_plugin");

    let mut ctx: HostContext = consume_context();
    let id = ctx
        .new_plugin(plugin_source, name)
        .await
        .map_err(|e| anyhow!("Plugin load fail: {:?}", e))?;

    info!("Loaded plugin {} [{}]", name, id);

    Ok(())
}

#[component]
fn events_toast_handler() -> Element {
    let ctx: HostContext = use_context();
    let toast_ctx: ToastContext = use_context();
    let mut last_count = use_signal(|| 0usize);

    use_effect(move || {
        let events = ctx.events();
        let new_count = events.len();
        let old_count = *last_count.peek();

        if new_count <= old_count {
            return;
        }

        for event in events.iter().skip(old_count) {
            match event.level {
                NotifyLevel::Trace => {}
                NotifyLevel::Info => {
                    toast_ctx.push(event.message.clone(), ToastKind::Info);
                }
                NotifyLevel::Error => {
                    toast_ctx.push(event.message.clone(), ToastKind::Error);
                }
            }
        }

        last_count.set(new_count);
    });

    rsx! {}
}
