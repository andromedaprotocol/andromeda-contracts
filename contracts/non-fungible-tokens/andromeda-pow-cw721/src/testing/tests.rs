use andromeda_non_fungible_tokens::cw721::{QueryMsg as AndrCw721QueryMsg, TokenExtension};
use andromeda_std::{amp::AndrAddr, common::encode_binary, error::ContractError};
use cosmwasm_std::Querier;
use cosmwasm_std::{from_json, to_json_binary, QueryRequest, WasmQuery};
use test_case::test_case;

use crate::testing::mock::{
    mint_pow_nft, proper_initialization, query_linked_cw721_address, query_pow_nft, submit_proof,
};
use crate::testing::mock_querier::{MOCK_CW721_CONTRACT, ORIGIN_MINTER};

pub const AUTHORIZED_ORIGIN_MINTER1: &str = "authorized_origin_minter1";
pub const AUTHORIZED_ORIGIN_MINTER2: &str = "authorized_origin_minter2";
pub const UNAUTHORIZED_ORIGIN_MINTER: &str = "unauthorized_origin_minter";

#[test]
fn test_instantiation() {
    let deps = proper_initialization(AndrAddr::from_string(MOCK_CW721_CONTRACT), None);
    let linked_cw721_address = query_linked_cw721_address(deps.as_ref())
        .unwrap()
        .linked_cw721_address;
    assert_eq!(
        linked_cw721_address,
        AndrAddr::from_string(MOCK_CW721_CONTRACT)
    );
}

#[test]
fn test_mint_pow_nft_invalid_user() {
    let mut deps = proper_initialization(
        AndrAddr::from_string(MOCK_CW721_CONTRACT),
        Some(vec![
            AndrAddr::from_string(AUTHORIZED_ORIGIN_MINTER1),
            AndrAddr::from_string(AUTHORIZED_ORIGIN_MINTER2),
        ]),
    );
    let err_response = mint_pow_nft(
        deps.as_mut(),
        UNAUTHORIZED_ORIGIN_MINTER,
        AndrAddr::from_string("owner"),
        "test_pow1".to_string(),
        None,
        TokenExtension {
            publisher: "Andromeda".to_string(),
        },
        10_u64,
    )
    .unwrap_err();

    assert_eq!(err_response, ContractError::Unauthorized {});
}

#[test]
fn test_mint_pow_nft() {
    let mut deps = proper_initialization(AndrAddr::from_string(MOCK_CW721_CONTRACT), None);

    mint_pow_nft(
        deps.as_mut(),
        ORIGIN_MINTER,
        AndrAddr::from_string(ORIGIN_MINTER),
        "test_pow1".to_string(),
        None,
        TokenExtension {
            publisher: "Andromeda".to_string(),
        },
        10_u64,
    )
    .unwrap();

    let owner_query_msg = to_json_binary(&QueryRequest::<cosmwasm_std::Empty>::Wasm(
        WasmQuery::Smart {
            contract_addr: MOCK_CW721_CONTRACT.to_string(),
            msg: encode_binary(&AndrCw721QueryMsg::OwnerOf {
                token_id: "test_pow1".to_string(),
                include_expired: None,
            })
            .unwrap(),
        },
    ))
    .unwrap();

    let raw_query_res = deps.querier.raw_query(&owner_query_msg);
    let owner_response: cw721::OwnerOfResponse =
        from_json(&(raw_query_res.unwrap()).unwrap()).unwrap();
    assert_eq!(owner_response.owner, ORIGIN_MINTER);

    let pow_nft = query_pow_nft(deps.as_ref(), "test_pow1".to_string()).unwrap();
    assert_eq!(pow_nft.nft_response.level, 1);
}

#[test_case("test_pow1", 20_u64, 582586_u128 ; "Difficulty: 20")]
#[test_case("test_pow1", 10_u64, 944_u128 ; "Difficulty: 10")]
#[test_case("test_pow1", 2_u64, 19_u128 ; "Difficulty: 2")]
fn test_submit_valid_proofs(token_id: &str, difficulty: u64, nonce: u128) {
    let mut deps = proper_initialization(AndrAddr::from_string(MOCK_CW721_CONTRACT), None);

    mint_pow_nft(
        deps.as_mut(),
        ORIGIN_MINTER,
        AndrAddr::from_string(ORIGIN_MINTER),
        token_id.to_string(),
        None,
        TokenExtension {
            publisher: "Andromeda".to_string(),
        },
        difficulty,
    )
    .unwrap();

    submit_proof(deps.as_mut(), "viewer", "test_pow1".to_string(), nonce).unwrap();

    let pow_nft = query_pow_nft(deps.as_ref(), token_id.to_string()).unwrap();

    assert_eq!(2, pow_nft.nft_response.level);
}

#[test_case("test_pow1", 20_u64, 58256_u128 ; "Difficulty: 20")]
#[test_case("test_pow1", 10_u64, 94_u128 ; "Difficulty: 10")]
#[test_case("test_pow1", 2_u64, 10_u128 ; "Difficulty: 2")]
fn test_submit_invalid_proofs(token_id: &str, difficulty: u64, nonce: u128) {
    let mut deps = proper_initialization(AndrAddr::from_string(MOCK_CW721_CONTRACT), None);

    mint_pow_nft(
        deps.as_mut(),
        ORIGIN_MINTER,
        AndrAddr::from_string(ORIGIN_MINTER),
        token_id.to_string(),
        None,
        TokenExtension {
            publisher: "Andromeda".to_string(),
        },
        difficulty,
    )
    .unwrap();

    let err = submit_proof(deps.as_mut(), "viewer", "test_pow1".to_string(), nonce).unwrap_err();

    assert_eq!(
        err,
        ContractError::CustomError {
            msg: "Proof does not meet difficulty".to_string()
        }
    );
}

#[test]
fn test_increase_level() {
    let mut deps = proper_initialization(AndrAddr::from_string(MOCK_CW721_CONTRACT), None);

    mint_pow_nft(
        deps.as_mut(),
        ORIGIN_MINTER,
        AndrAddr::from_string(ORIGIN_MINTER),
        "test_pow1".to_string(),
        None,
        TokenExtension {
            publisher: "Andromeda".to_string(),
        },
        2_u64,
    )
    .unwrap();

    let nonces_to_submit = vec![19_u128, 5_u128, 0_u128, 50_u128, 1474_u128, 16440_u128];

    for nonce in nonces_to_submit.iter() {
        submit_proof(deps.as_mut(), "viewer", "test_pow1".to_string(), *nonce).unwrap();
    }

    let pow_nft = query_pow_nft(deps.as_ref(), "test_pow1".to_string()).unwrap();
    assert_eq!(7, pow_nft.nft_response.level);
}
