use dioxus::prelude::*;

pub mod components;
pub mod contexts;

#[derive(PartialEq, Clone)]
pub enum Component {
    Container { children: Vec<Component> },
    Heading { text: String },
    Text { text: String },
    Button { text: String, event: String },
}

#[derive(PartialEq, Clone, Props)]
pub struct ComponentProps {
    pub component: Component,
}

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
        Component::Button { text, event } => {
            rsx! {
                button {
                    class: "btn btn-primary",
                    onclick: move |_| println!("Button clicked: {}", event),
                    "{text}"
                }
            }
        }
    }
}
