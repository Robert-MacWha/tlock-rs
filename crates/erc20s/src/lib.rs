use alloy::primitives::{Address, address};
use tlock_api::caip::AssetId;

#[derive(Clone)]
pub struct ERC20 {
    pub address: Address,
    pub asset_id: AssetId,
    pub chain_id: u64,
    pub symbol: &'static str,
    pub slot: u64,
}

pub const ERC20S: [ERC20; 3] = [
    ERC20 {
        address: address!("0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14"),
        asset_id: AssetId::erc20(
            111555111,
            address!("0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14"),
        ),
        chain_id: 111555111,
        symbol: "WETH",
        slot: 3,
    },
    ERC20 {
        address: address!("0xaA8E23Fb1079EA71e0a56F48a2aA51851D8433D0"),
        asset_id: AssetId::erc20(
            111555111,
            address!("0xaA8E23Fb1079EA71e0a56F48a2aA51851D8433D0"),
        ),
        chain_id: 111555111,
        symbol: "USDT",
        slot: 0,
    },
    ERC20 {
        address: address!("0xd7b45cbc28ba9ba8653665d5fb37167a2afe35d9"),
        asset_id: AssetId::erc20(
            111555111,
            address!("0xd7b45cbc28ba9ba8653665d5fb37167a2afe35d9"),
        ),
        chain_id: 111555111,
        symbol: "UNI",
        slot: 0,
    },
];

pub fn get_erc20_by_address(address: &Address) -> Option<ERC20> {
    for erc20 in ERC20S.iter() {
        if &erc20.address == address {
            return Some(erc20.clone());
        }
    }
    None
}

pub fn get_erc20_by_symbol(symbol: &str) -> Option<ERC20> {
    for erc20 in ERC20S.iter() {
        if erc20.symbol == symbol {
            return Some(erc20.clone());
        }
    }
    None
}
