use std::sync::{Arc, atomic::AtomicU64};

use futures::lock::Mutex;

use crate::{api::TlockApi, json_rpc_transport::JsonRpcTransport, transport::RpcMessage};

pub struct TypedHost {
    id: AtomicU64,
    transport: JsonRpcTransport,
}

impl TypedHost {
    pub fn new(transport: JsonRpcTransport) -> Self {
        Self {
            id: AtomicU64::new(0),
            transport,
        }
    }
}

impl TlockApi for TypedHost {
    fn ping(&self, value: &str) -> String {
        let id = self.id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        //? handler can be none because this plugin should only ever receive a
        //? single request from the host, so there will be no `RpcMessage::Request`s
        //? to handle
        let result = self
            .transport
            .call(id, "tlock_ping", value.into(), None)
            .unwrap();

        match result {
            RpcMessage::ResponseOk { result, .. } => result.as_str().unwrap().to_string(),
            _ => panic!("Unexpected message type"),
        }
    }

    fn version(&self) -> String {
        todo!()
    }
}
