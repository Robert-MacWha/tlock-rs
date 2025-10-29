use dioxus::prelude::*;
use tlock_hdk::tlock_api::{component::Component, page::PageEvent};

#[derive(PartialEq, Clone, Props)]
pub struct ComponentProps {
    component: Component,
    on_event: Callback<PageEvent, ()>,
}

/// Render a UI component recursively.
#[component]
pub fn RenderComponent(props: ComponentProps) -> Element {
    let component = props.component;
    match component {
        Component::Container { children } => {
            rsx! {
                div {
                    class: "container",
                    {
                        children
                            .iter()
                            .map(|child| rsx!(RenderComponent { component: child.clone(), on_event: props.on_event }))
                    }
                }
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
        Component::TextInput { placeholder, id } => {
            rsx! {
                input {
                    class: "form-control",
                    r#type: "text",
                    name: "{id}",
                    placeholder: "{placeholder}",
                }
            }
        }
        Component::Form { fields, id } => {
            rsx! {
                form {
                    class: "form",
                    onsubmit: move |e| {
                        let data = e.data().clone().values();
                        let data = data.iter().map(|(k, v)| (k.clone(), v.0.clone())).collect();
                        props.on_event.call(PageEvent::FormSubmitted(id.clone(), data));
                    },
                    {
                        fields
                            .iter()
                            .map(|field| rsx!(RenderComponent { component: field.clone(), on_event: props.on_event }))
                    }
                }
            }
        }
        Component::SubmitInput { text } => {
            rsx! {
                button {
                    class: "btn btn-primary",
                    r#type: "submit",
                    "{text}",
                }
            }
        }
    }
}
