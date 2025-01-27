use andromeda_std::{amp::AndrAddr, andr_exec, andr_instantiate, andr_query};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Binary};

use crate::cw721::TokenExtension;

#[andr_instantiate]
#[cw_serde]
pub struct InstantiateMsg {
    pub linked_cw721_address: AndrAddr,
    pub authorized_origin_minter_addresses: Option<Vec<AndrAddr>>,
}

#[andr_exec]
#[cw_serde]
pub enum ExecuteMsg {
    MintPowNFT {
        owner: AndrAddr,
        token_id: String,
        token_uri: Option<String>,
        extension: TokenExtension,
        base_difficulty: u64,
    },
    SubmitProof {
        token_id: String,
        solution: u128,
    },
}

#[cw_serde]
pub struct PowNFTInfo {
    pub owner: Addr,
    pub level: u64,
    pub last_hash: Binary,
    pub difficulty: u64,
}

#[andr_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(GetPowNFTResponse)]
    GetPowNFT { token_id: String },
    #[returns(GetLinkedCw721AddressResponse)]
    GetLinkedCw721Address {},
}

#[cw_serde]
pub struct GetPowNFTResponse {
    pub nft_response: PowNFTInfo,
}

#[cw_serde]
pub struct GetLinkedCw721AddressResponse {
    pub linked_cw721_address: AndrAddr,
}
