use serde::{Deserialize, Serialize};

/// Basic UI component templates that can be used by plugins to build pages
/// and other UI custom elements.
#[derive(PartialEq, Clone, Serialize, Deserialize, Debug)]
pub enum Component {
    Container {
        children: Vec<Component>,
    },
    Heading {
        text: String,
    },
    Heading2 {
        text: String,
    },
    Text {
        text: String,
    },
    UnorderedList {
        items: Vec<(String, Component)>,
    },
    ButtonInput {
        text: String,
        id: String,
    },
    Form {
        fields: Vec<Component>,
        id: String,
    },
    TextInput {
        placeholder: String,
        id: String,
    },
    SubmitInput {
        text: String,
    },
    Dropdown {
        id: String,
        options: Vec<String>,
        selected: Option<String>,
    },
}

impl Component {
    pub fn empty() -> Self {
        Component::Container { children: vec![] }
    }
}

impl From<&str> for Component {
    fn from(s: &str) -> Self {
        Component::Text {
            text: s.to_string(),
        }
    }
}

impl From<String> for Component {
    fn from(s: String) -> Self {
        Component::Text { text: s }
    }
}

pub fn container<I>(children: I) -> Component
where
    I: IntoIterator<Item = Component>,
{
    Component::Container {
        children: children.into_iter().collect(),
    }
}

pub fn heading(text: impl Into<String>) -> Component {
    Component::Heading { text: text.into() }
}

pub fn heading2(text: impl Into<String>) -> Component {
    Component::Heading2 { text: text.into() }
}

pub fn text(text: impl Into<String>) -> Component {
    Component::Text { text: text.into() }
}

pub fn unordered_list<I, S>(items: I) -> Component
where
    I: IntoIterator<Item = (S, Component)>,
    S: Into<String>,
{
    let items = items.into_iter().map(|(k, v)| (k.into(), v)).collect();
    Component::UnorderedList { items }
}

pub fn button_input(id: impl Into<String>, text: impl Into<String>) -> Component {
    Component::ButtonInput {
        id: id.into(),
        text: text.into(),
    }
}

pub fn form<I>(id: impl Into<String>, fields: I) -> Component
where
    I: IntoIterator<Item = Component>,
{
    Component::Form {
        id: id.into(),
        fields: fields.into_iter().collect(),
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

pub fn dropdown<I, S>(id: impl Into<String>, options: I, selected: Option<S>) -> Component
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let options = options.into_iter().map(|s| s.into()).collect();
    let selected = selected.map(|s| s.into());
    Component::Dropdown {
        id: id.into(),
        options,
        selected,
    }
}
