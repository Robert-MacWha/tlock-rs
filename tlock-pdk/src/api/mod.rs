pub trait TlockApi {
    fn ping(&mut self, value: &str) -> String;
    fn version(&mut self) -> String;
}
