use andromeda_non_fungible_tokens::pow_cw721::PowNFTInfo;
use andromeda_std::amp::AndrAddr;
use cw_storage_plus::{Item, Map};

pub const LINKED_CW721_ADDRESS: Item<AndrAddr> = Item::new("linked_cw721_address");
pub const POW_NFT: Map<String, PowNFTInfo> = Map::new("pow_nft");
