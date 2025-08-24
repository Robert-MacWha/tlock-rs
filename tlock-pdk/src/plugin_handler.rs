use crate::{
    api::TlockApi,
    transport::{RequestHandler, Transport},
};

pub trait PluginHandler: RequestHandler + TlockApi {}

impl<T> RequestHandler for T
where
    T: PluginHandler,
{
    fn handle(
        &mut self,
        method: &str,
        params: serde_json::Value,
        _transport: &mut dyn Transport,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        match method {
            "tlock_ping" => Ok(self.ping(params.as_str().unwrap()).into()),
            "tlock_version" => Ok(self.version().into()),
            _ => Err("Unknown method".into()),
        }
    }
}
