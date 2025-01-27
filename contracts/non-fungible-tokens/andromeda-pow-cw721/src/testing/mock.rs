// use andromeda_data_storage::graph::{Coordinate, GetMapInfoResponse, MapInfo};
use andromeda_non_fungible_tokens::cw721::TokenExtension;
use andromeda_non_fungible_tokens::pow_cw721::{
    ExecuteMsg, GetLinkedCw721AddressResponse, GetPowNFTResponse, InstantiateMsg, QueryMsg,
};
use andromeda_std::{
    amp::AndrAddr, error::ContractError, testing::mock_querier::MOCK_KERNEL_CONTRACT,
};
use cosmwasm_std::{
    from_json,
    testing::{mock_env, mock_info, MockApi, MockStorage},
    Deps, DepsMut, OwnedDeps, Response,
};

use crate::contract::{execute, instantiate, query};
use crate::testing::mock_querier::{mock_dependencies_custom, WasmMockQuerier};

pub type MockDeps = OwnedDeps<MockStorage, MockApi, WasmMockQuerier>;

pub fn proper_initialization(
    linked_cw721_address: AndrAddr,
    authorized_origin_minter_addresses: Option<Vec<AndrAddr>>,
) -> MockDeps {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    let msg = InstantiateMsg {
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
        linked_cw721_address,
        authorized_origin_minter_addresses,
    };
    let env = mock_env();
    instantiate(deps.as_mut(), env, info, msg).unwrap();
    deps
}

pub fn mint_pow_nft(
    deps: DepsMut<'_>,
    sender: &str,
    owner: AndrAddr,
    token_id: String,
    token_uri: Option<String>,
    extension: TokenExtension,
    base_difficulty: u64,
) -> Result<Response, ContractError> {
    let msg = ExecuteMsg::MintPowNFT {
        owner,
        token_id,
        token_uri,
        extension,
        base_difficulty,
    };
    let info = mock_info(sender, &[]);
    execute(deps, mock_env(), info, msg)
}

pub fn submit_proof(
    deps: DepsMut<'_>,
    sender: &str,
    token_id: String,
    solution: u128,
) -> Result<Response, ContractError> {
    let msg = ExecuteMsg::SubmitProof { token_id, solution };
    let info = mock_info(sender, &[]);
    execute(deps, mock_env(), info, msg)
}

pub fn query_linked_cw721_address(
    deps: Deps,
) -> Result<GetLinkedCw721AddressResponse, ContractError> {
    let res = query(deps, mock_env(), QueryMsg::GetLinkedCw721Address {});
    match res {
        Ok(res) => Ok(from_json(res).unwrap()),
        Err(err) => Err(err),
    }
}

pub fn query_pow_nft(deps: Deps, token_id: String) -> Result<GetPowNFTResponse, ContractError> {
    let res = query(deps, mock_env(), QueryMsg::GetPowNFT { token_id });
    match res {
        Ok(res) => Ok(from_json(res).unwrap()),
        Err(err) => Err(err),
    }
}
