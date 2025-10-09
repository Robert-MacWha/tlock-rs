use dioxus::prelude::*;
use dioxus_logger::tracing::info;
use tlock_hdk::tlock_api::component::Component;

#[derive(PartialEq, Clone, Props)]
pub struct ComponentProps {
    pub component: Component,
}

/// Render a UI component recursively.
#[component]
pub fn RenderComponent(props: ComponentProps) -> Element {
    let component = props.component;
    match component {
        Component::Container { children } => {
            rsx! {
                div { class: "container", {children.iter().map(|child| rsx!(RenderComponent { component: child.clone() }))} }
            }
        }
        Component::Heading { text } => {
            rsx! {
                h1 { "{text}" }
            }
        }
        Component::Text { text } => {
            rsx! {
                p { "{text}" }
            }
        }
        Component::Button { text, id } => {
            rsx! {
                button {
                    class: "btn btn-primary",
                    onclick: move |_| info!("Button clicked: {}", id),
                    "{text}"
                }
            }
        }
    }
}
