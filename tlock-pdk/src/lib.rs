use extism_pdk::{FromBytes, Json, ToBytes};
use serde::{Deserialize, Serialize};

#[derive(FromBytes, ToBytes, Deserialize, Serialize, PartialEq, Debug)]
#[encoding(Json)]
pub struct Add {
    pub left: i32,
    pub right: i32,
}
#[derive(FromBytes, ToBytes, Deserialize, Serialize, PartialEq, Debug)]
#[encoding(Json)]
pub struct Sum {
    pub value: i32,
}
