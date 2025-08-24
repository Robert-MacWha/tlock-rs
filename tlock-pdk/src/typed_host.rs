use crate::{
    api::TlockApi,
    transport::{RequestHandler, RpcMessage, Transport},
};

pub struct TypedHost {
    transport: Box<dyn Transport>,
    handler: Box<dyn RequestHandler>,
}

impl TypedHost {
    pub fn new(transport: Box<dyn Transport>, handler: Box<dyn RequestHandler>) -> Self {
        Self { transport, handler }
    }
}

impl TlockApi for TypedHost {
    fn ping(&mut self, value: &str) -> String {
        let result = self
            .transport
            .call("tlock_ping", value.into(), self.handler.as_mut())
            .unwrap();

        match result {
            RpcMessage::ResponseOk { result, .. } => result.as_str().unwrap().to_string(),
            _ => panic!("Unexpected message type"),
        }
    }

    fn version(&mut self) -> String {
        todo!()
    }
}
