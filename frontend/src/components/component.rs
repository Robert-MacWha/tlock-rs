use alloy::primitives::U256;
use dioxus::prelude::*;
use tlock_hdk::tlock_api::{
    caip::{AccountAddress, AssetType},
    component::Component,
    page::PageEvent,
};
use web_sys::js_sys::eval;

fn format_balance(amount: U256, decimals: u8) -> String {
    let amount_f64 = amount.to_string().parse::<f64>().unwrap_or(0.0);
    format!("{:.4}", amount_f64 / 10_f64.powi(decimals as i32))
}

fn shorten_addr(addr: &str) -> String {
    format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
}

fn get_asset_info(asset_type: &AssetType) -> (String, u8) {
    match asset_type {
        AssetType::Slip44(60) => ("ETH".to_string(), 18),
        AssetType::Slip44(n) => (format!("slip44:{}", n), 18),
        AssetType::Erc20(addr) => erc20s::get_erc20_by_address(addr)
            .map(|t| (t.symbol.to_string(), t.decimals))
            .unwrap_or_else(|| (format!("erc20:{}", shorten_addr(&format!("{:?}", addr))), 18)),
        AssetType::Custom { namespace, reference } => (
            format!(
                "{}:{}...{}",
                namespace,
                &reference[..6.min(reference.len())],
                &reference[reference.len().saturating_sub(4)..]
            ),
            18,
        ),
    }
}

#[derive(PartialEq, Clone, Props)]
pub struct ComponentProps {
    component: Component,
    on_event: Callback<PageEvent, ()>,
}

#[component]
pub fn RenderComponent(props: ComponentProps) -> Element {
    let component = props.component;
    match component {
        Component::Container { children } => {
            rsx! {
                div { class: "flex flex-col items-start gap-2",
                    {children.iter().map(|child| rsx! {
                        RenderComponent { component: child.clone(), on_event: props.on_event }
                    })}
                }
            }
        }
        Component::Heading { text } => {
            rsx! {
                h1 { class: "text-xl font-bold mt-4", "{text}" }
            }
        }
        Component::Heading2 { text } => {
            rsx! {
                h2 { class: "text-lg font-semibold mt-3", "{text}" }
            }
        }
        Component::Text { text } => {
            rsx! {
                p { "{text}" }
            }
        }
        Component::UnorderedList { items } => {
            rsx! {
                ul {
                    for (key , item) in items {
                        li { class: "mb-2", key: "{key}",
                            RenderComponent {
                                component: item.clone(),
                                on_event: props.on_event,
                            }
                        }
                    }
                }
            }
        }
        Component::ButtonInput { text, id } => {
            rsx! {
                button {
                    class: "btn btn-primary",
                    onclick: move |_| {
                        props.on_event.call(PageEvent::ButtonClicked(id.clone()));
                    },
                    "{text}"
                }
            }
        }
        Component::TextInput {
            placeholder,
            label,
            id,
        } => {
            rsx! {
                fieldset { class: "fieldset",
                    label { class: "label", "{label}" }
                    input {
                        class: "input w-full",
                        name: "{id}",
                        r#type: "text",
                        placeholder: "{placeholder}",
                    }
                }
            }
        }
        Component::Form { fields, id } => {
            rsx! {
                form {
                    class: "flex flex-col gap-4 bg-base-100 p-4 rounded-box shadow-sm w-full",
                    onsubmit: move |e| {
                        e.prevent_default();
                        let data = e.data().clone().values();
                        let data = data
                            .iter()
                            .filter_map(|(k, v)| match v {
                                FormValue::Text(v) => Some((k.clone(), v.clone())),
                                _ => None,
                            })
                            .collect();
                        props.on_event.call(PageEvent::FormSubmitted(id.clone(), data));
                    },
                    {fields.iter().map(|field| rsx! {
                        RenderComponent { component: field.clone(), on_event: props.on_event }
                    })}
                }
            }
        }
        Component::SubmitInput { text } => {
            rsx! {
                div { class: "divider" }
                button { class: "btn btn-primary w-full mt-2", r#type: "submit", "{text}" }
            }
        }
        Component::DropdownInput {
            label,
            options,
            selected,
            id,
        } => {
            rsx! {
                fieldset { class: "fieldset",
                    label { class: "label", "{label}" }
                    select { class: "select w-full", name: "{id}",
                        {
                            options
                                .iter()
                                .map(|option| {
                                    let is_selected = selected.as_ref() == Some(option);
                                    rsx! {
                                        option { value: "{option}", selected: is_selected, "{option}" }
                                    }
                                })
                        }
                    }
                }
            }
        }
        Component::Chain { id } => {
            rsx! {
                div { class: "join border border-base-300 rounded-lg",
                    div { class: "join-item px-3 py-1 font-mono text-sm flex items-center",
                        "{id}"
                    }
                    button {
                        class: "join-item btn btn-ghost btn-sm border-l border-base-300",
                        onclick: move |_| {
                            let _ = eval(&format!("navigator.clipboard.writeText('{}')", id));
                        },
                        "Copy"
                    }
                }
            }
        }
        Component::Account { id } => {
            let addr = match &id.address {
                AccountAddress::Evm(a) => format!("{:?}", a),
                AccountAddress::Custom(s) => s.clone(),
            };

            rsx! {
                div { class: "join border border-base-300 rounded-lg",
                    div { class: "join-item px-3 py-1 font-mono text-sm flex items-center",
                        "{id.chain_id.namespace()}:{id.chain_id.reference().unwrap_or_else(|| \"_\".to_string())}"
                    }
                    div {
                        class: "join-item px-3 py-1 font-mono text-sm flex items-center tooltip cursor-help before:max-w-md",
                        "data-tip": "{addr}",
                        "{shorten_addr(&addr)}"
                    }
                    button {
                        class: "join-item btn btn-ghost btn-sm border-l border-base-300",
                        onclick: move |_| {
                            let _ = eval(&format!("navigator.clipboard.writeText('{}')", id));
                        },
                        "Copy"
                    }
                }
            }
        }
        Component::Asset { id, balance } => {
            let (asset_display, decimals) = get_asset_info(&id.asset);

            rsx! {
                div { class: "join border border-base-300 rounded-lg",
                    div { class: "join-item px-3 py-1 font-mono text-sm flex items-center",
                        "{id.chain_id.namespace()}:{id.chain_id.reference().unwrap_or_else(|| \"_\".to_string())}"
                    }
                    div {
                        class: "join-item px-3 py-1 font-mono text-sm flex items-center tooltip cursor-help before:max-w-md",
                        "data-tip": "{id.asset}",
                        "{asset_display}"
                    }
                    if let Some(bal) = balance {
                        div { class: "join-item px-3 py-1 font-mono text-sm flex items-center",
                            {format_balance(bal, decimals)}
                        }
                    }
                    button {
                        class: "join-item btn btn-ghost btn-sm border-l border-base-300",
                        onclick: move |_| {
                            let _ = eval(&format!("navigator.clipboard.writeText('{}')", id));
                        },
                        "Copy"
                    }
                }
            }
        }
        Component::EntityId { id } => {
            let id = id.to_string();
            let (t, uuid) = id.split_once(":").unwrap_or(("", id.as_str()));

            rsx! {
                div { class: "join border border-base-300 rounded-lg",
                    div { class: "join-item px-3 py-1 font-mono text-sm flex items-center",
                        "{t}"
                    }
                    div { class: "join-item px-3 py-1 font-mono text-sm flex items-center",
                        "{uuid}"
                    }
                    button {
                        class: "join-item btn btn-ghost btn-sm border-l border-base-300",
                        onclick: move |_| {
                            let _ = eval(&format!("navigator.clipboard.writeText('{}')", id));
                        },
                        "Copy"
                    }
                }
            }
        }
        Component::Hex { data } => {
            let hex_str = format!("0x{}", hex::encode(&data));
            rsx! {
                div { class: "join border border-base-300 rounded-lg",
                    div { class: "join-item px-3 py-1 font-mono text-sm flex items-center",
                        span { class: "w-24 truncate", "{hex_str}" }
                    }
                    button {
                        class: "join-item btn btn-ghost btn-sm border-l border-base-300",
                        onclick: move |_| {
                            let _ = eval(&format!("navigator.clipboard.writeText('{}')", hex_str));
                        },
                        "Copy"
                    }
                }
            }
        }
    }
}
