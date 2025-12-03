use serde::{Deserialize, Serialize};

/// Basic UI component templates that can be used by plugins to build pages
/// and other UI custom elements.
#[derive(PartialEq, Clone, Serialize, Deserialize, Debug)]
pub enum Component {
    Container { children: Vec<Component> },
    Heading { text: String },
    Text { text: String },
    ButtonInput { text: String, id: String },
    Form { fields: Vec<Component>, id: String },
    TextInput { placeholder: String, id: String },
    SubmitInput { text: String },
    Dropdown { id: String, options: Vec<String>, selected: Option<String> },
}

impl Component {
    pub fn empty() -> Self {
        Component::Container { children: vec![] }
    }
}

pub fn container(children: Vec<Component>) -> Component {
    Component::Container { children }
}

pub fn heading(text: impl Into<String>) -> Component {
    Component::Heading { text: text.into() }
}

pub fn text(text: impl Into<String>) -> Component {
    Component::Text { text: text.into() }
}

pub fn button_input(id: impl Into<String>, text: impl Into<String>) -> Component {
    Component::ButtonInput {
        id: id.into(),
        text: text.into(),
    }
}

pub fn form(id: impl Into<String>, fields: Vec<Component>) -> Component {
    Component::Form {
        id: id.into(),
        fields,
    }
}

pub fn text_input(id: impl Into<String>, placeholder: impl Into<String>) -> Component {
    Component::TextInput {
        id: id.into(),
        placeholder: placeholder.into(),
    }
}

pub fn submit_input(text: impl Into<String>) -> Component {
    Component::SubmitInput { text: text.into() }
}

pub fn dropdown(id: impl Into<String>, options: Vec<String>, selected: Option<String>) -> Component {
    Component::Dropdown {
        id: id.into(),
        options,
        selected,
    }
}
