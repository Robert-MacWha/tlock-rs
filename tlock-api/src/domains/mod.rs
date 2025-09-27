use serde::{Deserialize, Serialize};

pub mod eip155_keyring;
pub mod host;
pub mod tlock;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Domains {
    Eip155Keyring,
    Plugin,
    Tlock,
    Host,
}
