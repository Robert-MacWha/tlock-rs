use dioxus::prelude::*;

#[derive(PartialEq, Clone, Props)]
struct ContainerProps {
    pub children: Element,
}

#[component]
fn Container(props: ContainerProps) -> Element {
    rsx! {
        div { class: "container", {props.children} }
    }
}
