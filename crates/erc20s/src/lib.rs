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

pub const CHAIN_ID: u64 = 1;

pub const ERC20S: [ERC20; 3] = [
    ERC20 {
        address: address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
        asset_id: AssetId::erc20(
            CHAIN_ID,
            address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
        ),
        chain_id: CHAIN_ID,
        symbol: "WETH",
        slot: 3,
    },
    ERC20 {
        address: address!("0x6b175474e89094c44da98b954eedeac495271d0f"),
        asset_id: AssetId::erc20(
            CHAIN_ID,
            address!("0x6b175474e89094c44da98b954eedeac495271d0f"),
        ),
        chain_id: CHAIN_ID,
        symbol: "DAI",
        slot: 2,
    },
    ERC20 {
        address: address!("0xDe30da39c46104798bB5aA3fe8B9e0e1F348163F"),
        asset_id: AssetId::erc20(
            CHAIN_ID,
            address!("0xDe30da39c46104798bB5aA3fe8B9e0e1F348163F"),
        ),
        chain_id: CHAIN_ID,
        symbol: "GTC",
        slot: 5,
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
