pub trait TlockApi {
    fn ping(&self, value: &str) -> String;
    fn version(&self) -> String;
}
