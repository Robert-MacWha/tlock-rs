// use crate::{
//     api::TlockApi,
//     plugin::Plugin,
//     transport::{RequestHandler, RpcMessage, Transport},
// };

// /// TypedPlugin is a plugin for the host with typed wrappers for all valid functions
// pub struct TypedPlugin {
//     plugin: Plugin,
//     handler: Box<dyn RequestHandler>,
// }

// impl TypedPlugin {
//     pub fn new(plugin: Plugin, handler: Box<dyn RequestHandler>) -> Self {
//         Self { plugin, handler }
//     }
// }

// impl TlockApi for TypedPlugin {
//     fn ping(&self, value: &str) -> String {
//         let result = self
//             .plugin
//             .call("tlock_ping", value.into(), self.handler.as_ref())
//             .unwrap();

//         match result {
//             RpcMessage::ResponseOk { result, .. } => result.as_str().unwrap().to_string(),
//             _ => panic!("Unexpected message type"),
//         }
//     }

//     fn version(&self) -> String {
//         todo!()
//     }
// }
