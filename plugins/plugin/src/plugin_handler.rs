use crate::request_handler::RequestHandler;

pub trait PluginHandler: RequestHandler + TlockApi {}

impl<T> RequestHandler for T
where
    T: PluginHandler,
{
    fn handle(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        match method {
            "tlock_ping" => Ok(self.ping(params.as_str().unwrap()).into()),
            "tlock_version" => Ok(self.version().into()),
            _ => Err("Unknown method".into()),
        }
    }
}

pub trait TlockApi {
    fn ping(&self, value: &str) -> String;
    fn version(&self) -> String;
}
