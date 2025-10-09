use serde::{Deserialize, Serialize};

/// Basic UI component templates that can be used by plugins to build pages
/// and other UI custom elements.
#[derive(PartialEq, Clone, Serialize, Deserialize, Debug)]
pub enum Component {
    Container { children: Vec<Component> },
    Heading { text: String },
    Text { text: String },
    Button { text: String, id: u32 },
}

impl Component {
    pub fn empty() -> Self {
        Component::Container { children: vec![] }
    }
}
