use tlock_pdk::{api::TlockApi, transport::RpcMessage};

use crate::plugin::Plugin;

/// TypedPlugin is a type-safe wrapped plugin
pub struct TypedPlugin<'a> {
    plugin: Plugin<'a>,
}

impl<'a> TypedPlugin<'a> {
    pub fn new(plugin: Plugin<'a>) -> Self {
        Self { plugin }
    }
}

// TODO: Make a macro to generate this + typed_host
impl TlockApi for TypedPlugin<'_> {
    fn ping(&self, value: &str) -> String {
        let result = self.plugin.call("tlock_ping", value.into()).unwrap();
        match result {
            RpcMessage::ResponseOk { result, .. } => result.as_str().unwrap().to_string(),
            _ => panic!("Unexpected message type"),
        }
    }

    fn version(&self) -> String {
        todo!()
    }
}
