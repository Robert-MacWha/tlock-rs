use std::sync::Arc;

use anyhow::anyhow;
use dioxus::{
    logger::tracing::{error, info},
    prelude::*,
};
use frontend::{
    components::{page::Page, user_requests::UserRequestComponent},
    contexts::{
        host::HostContext,
        toast::{ToastContext, ToastKind, toast_container},
    },
};
use host::{host::Host, host_state::PluginSource};
use tlock_hdk::tlock_api::{
    self,
    entities::{EntityId, PageId},
    host::NotifyLevel,
};

#[derive(Copy, Clone)]
struct UiContext {
    show_request_sidebar: Signal<bool>,
    show_events_sidebar: Signal<bool>,
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
                div { class: "px-3 py-1.5",
                    fieldset { class: "fieldset",
                        legend { class: "fieldset-legend", "Load WASM Plugin" }
                        input {
                            class: "file-input file-input-sm px-0 py-0",
                            r#type: "file",
                            accept: ".wasm",
                            onchange: move |e| async move {
                                let toast_ctx: ToastContext = use_context();
                                if let Err(e) = handle_wasm_upload(e).await {
                                    error!("WASM upload failed: {:?}", e);
                                    toast_ctx.push(format!("WASM upload failed: {:?}", e), ToastKind::Error);
                                }
                            },
                        }
                    }
                }
            }
        }
    }
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

async fn handle_wasm_upload(e: Event<FormData>) -> anyhow::Result<()> {
    let files = e.files();
    let file = files.first().context("No file selected")?;
    let name = file.name();
    let data = file
        .read_bytes()
        .await
        .map_err(|e| anyhow!("Read fail {}: {:?}", name, e))?;

    let plugin_source = PluginSource::Embedded(data.to_vec());
    let name = name.strip_suffix(".wasm").unwrap_or(&name);

    let mut ctx: HostContext = consume_context();
    let id = ctx
        .new_plugin(plugin_source, name)
        .await
        .map_err(|e| anyhow!("Plugin load fail: {:?}", e))?;

    info!("Loaded plugin {} [{}]", name, id);
    consume_context::<ToastContext>().push(format!("Loaded {}", name), ToastKind::Success);

    Ok(())
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
