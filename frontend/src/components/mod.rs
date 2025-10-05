use dioxus::prelude::*;

#[derive(PartialEq, Clone, Props)]
struct ContainerProps {
    children: Element,
}

#[component]
fn Container(props: ContainerProps) -> Element {
    rsx! {
        div { class: "container", {props.children} }
    }
}
