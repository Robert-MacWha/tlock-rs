/// Entity IDs are used to uniquely identify entities managed by plugins. Each
/// entity ID is comprised of its domain (e.g. `Vault`) and a unique per-domain
/// identifier (e.g. `"0xabc123..."`).
pub type EntityId = String;

pub trait ToEntityId {
    fn to_id(&self) -> EntityId;
}

pub type VaultId = String;
impl ToEntityId for VaultId {
    fn to_id(&self) -> EntityId {
        format!("vault:{}", self)
    }
}
