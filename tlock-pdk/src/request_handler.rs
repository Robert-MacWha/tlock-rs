use serde_json::Value;

pub trait RequestHandler {
    fn handle(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>;
}
