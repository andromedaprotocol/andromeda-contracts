use andromeda_non_fungible_tokens::marketplace::{
    Cw20HookMsg, Cw721HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, Status,
};
use andromeda_std::{
    ado_base::{
        permissioning::{LocalPermission, Permission},
        rates::{LocalRate, LocalRateType, LocalRateValue, PercentRate, Rate},
    },
    ado_contract::ADOContract,
    amp::{AndrAddr, Recipient},
    common::{
        denom::{
            Asset, AuthorizedAddressesResponse, PermissionAction, SEND_CW20_ACTION, SEND_NFT_ACTION,
        },
        encode_binary,
        expiration::{expiration_from_milliseconds, Expiry, MILLISECONDS_TO_NANOSECONDS_RATIO},
        Milliseconds,
    },
    error::ContractError,
    testing::mock_querier::MOCK_CW20_CONTRACT,
};
use cosmwasm_std::{
    attr, coin, coins, from_json,
    testing::{mock_env, mock_info},
    BankMsg, CosmosMsg, Decimal, Deps, DepsMut, Env, Response, SubMsg, Uint128, WasmMsg,
};
use cw20::Cw20ReceiveMsg;
use cw721::{Cw721ExecuteMsg, Cw721ReceiveMsg};
use cw_utils::Expiration;

use super::mock_querier::MOCK_KERNEL_CONTRACT;
use crate::{
    contract::{execute, instantiate, query},
    state::{sale_infos, SaleInfo, TokenSaleState, TOKEN_SALE_STATE},
    testing::mock_querier::{
        mock_dependencies_custom, MOCK_CW721_ADDR, MOCK_TOKEN_ADDR, MOCK_TOKEN_OWNER,
        MOCK_UNCLAIMED_TOKEN,
    },
};

fn start_sale(deps: DepsMut, coin_denom: Asset) {
    let hook_msg = Cw721HookMsg::StartSale {
        coin_denom,
        price: Uint128::new(100),
        start_time: None,
        duration: None,
        recipient: None,
    };
    let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_owned(),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        msg: encode_binary(&hook_msg).unwrap(),
    });
    let env = mock_env();

    let info = mock_info(MOCK_TOKEN_ADDR, &[]);
    let _res = execute(deps, env, info, msg).unwrap();
}

fn start_sale_future_start(deps: DepsMut, env: Env, coin_denom: Asset) {
    let current_time = env.block.time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO;
    let hook_msg = Cw721HookMsg::StartSale {
        coin_denom,
        price: Uint128::new(100),
        // Add one to the current time to have it set in the future
        start_time: Some(Expiry::AtTime(Milliseconds(current_time + 1))),
        duration: None,
        recipient: None,
    };
    let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_owned(),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        msg: encode_binary(&hook_msg).unwrap(),
    });
    let env = mock_env();

    let info = mock_info(MOCK_TOKEN_ADDR, &[]);
    let _res = execute(deps, env, info, msg).unwrap();
}

fn start_sale_future_start_with_duration(deps: DepsMut, env: Env) {
    let current_time = env.block.time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO;
    let hook_msg = Cw721HookMsg::StartSale {
        coin_denom: Asset::NativeToken("uusd".to_string()),
        price: Uint128::new(100),
        // Add one to the current time to have it set in the future
        start_time: Some(Expiry::AtTime(Milliseconds(current_time + 1))),
        // Add duration, the end time's expiration will be current time + duration
        duration: Some(Milliseconds(1)),
        recipient: None,
    };
    let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_owned(),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        msg: encode_binary(&hook_msg).unwrap(),
    });
    let env = mock_env();

    let info = mock_info(MOCK_TOKEN_ADDR, &[]);
    let _res = execute(deps, env, info, msg).unwrap();
}

fn init(
    deps: DepsMut,
    authorized_cw20_addresses: Option<Vec<AndrAddr>>,
    authorized_token_addresses: Option<Vec<AndrAddr>>,
) -> Response {
    let msg = InstantiateMsg {
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        authorized_cw20_addresses,
        authorized_token_addresses,
    };

    let info = mock_info("owner", &[]);
    instantiate(deps, mock_env(), info, msg).unwrap()
}

fn assert_sale_created(deps: Deps, env: Env, coin_denom: String, uses_cw20: bool) {
    let current_time = env.block.time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO;
    let start_time_expiration =
        expiration_from_milliseconds(Milliseconds(current_time + 1)).unwrap();
    assert_eq!(
        TokenSaleState {
            coin_denom,
            sale_id: 1u128.into(),
            owner: MOCK_TOKEN_OWNER.to_string(),
            token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
            token_address: MOCK_TOKEN_ADDR.to_owned(),
            status: Status::Open,
            price: Uint128::new(100),
            // start sale function has start_time set as None, so it defaults to the current time
            start_time: start_time_expiration,
            end_time: Expiration::Never {},
            uses_cw20,
            recipient: None,
        },
        TOKEN_SALE_STATE.load(deps.storage, 1u128).unwrap()
    );

    assert_eq!(
        SaleInfo {
            sale_ids: vec![Uint128::from(1u128)],
            token_address: MOCK_TOKEN_ADDR.to_owned(),
            token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        },
        sale_infos()
            .load(
                deps.storage,
                &(MOCK_UNCLAIMED_TOKEN.to_owned() + MOCK_TOKEN_ADDR)
            )
            .unwrap()
    );
}

fn assert_sale_created_future_start(deps: Deps, env: Env, coin_denom: String, uses_cw20: bool) {
    let current_time = env.block.time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO;
    // Add one to the current time to have it set in the future
    let start_time_expiration =
        expiration_from_milliseconds(Milliseconds(current_time + 1)).unwrap();
    assert_eq!(
        TokenSaleState {
            coin_denom,
            sale_id: 1u128.into(),
            owner: MOCK_TOKEN_OWNER.to_string(),
            token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
            token_address: MOCK_TOKEN_ADDR.to_owned(),
            status: Status::Open,
            price: Uint128::new(100),
            start_time: start_time_expiration,
            end_time: Expiration::Never {},
            uses_cw20,
            recipient: None,
        },
        TOKEN_SALE_STATE.load(deps.storage, 1u128).unwrap()
    );

    assert_eq!(
        SaleInfo {
            sale_ids: vec![Uint128::from(1u128)],
            token_address: MOCK_TOKEN_ADDR.to_owned(),
            token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        },
        sale_infos()
            .load(
                deps.storage,
                &(MOCK_UNCLAIMED_TOKEN.to_owned() + MOCK_TOKEN_ADDR)
            )
            .unwrap()
    );
}

#[test]
fn test_sale_instantiate() {
    let mut deps = mock_dependencies_custom(&[]);
    let res = init(deps.as_mut(), None, None);
    assert_eq!(0, res.messages.len());
}

#[test]
fn test_instantiate_with_multiple_authorized_cw20_addresses() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);

    let authorized_cw20_addresses = vec![
        AndrAddr::from_string("cw20_contract_1"),
        AndrAddr::from_string("cw20_contract_2"),
        AndrAddr::from_string("cw20_contract_3"),
    ];

    let msg = InstantiateMsg {
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
        authorized_token_addresses: None,
        authorized_cw20_addresses: Some(authorized_cw20_addresses.clone()),
    };

    let res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // Check if each authorized CW20 address has the correct permission
    for addr in authorized_cw20_addresses {
        let raw_addr = addr.get_raw_address(&deps.as_ref()).unwrap();
        let permission =
            ADOContract::get_permission(deps.as_ref().storage, SEND_CW20_ACTION, raw_addr).unwrap();
        assert_eq!(
            permission,
            Some(Permission::Local(LocalPermission::Whitelisted(None)))
        );
    }

    // Check that a non-authorized address doesn't have permission
    let non_authorized = "non_authorized_cw20".to_string();
    let permission =
        ADOContract::get_permission(deps.as_ref().storage, SEND_CW20_ACTION, non_authorized)
            .unwrap();
    assert_eq!(permission, None);
}

#[test]
fn test_sale_instantiate_future_start() {
    let mut deps = mock_dependencies_custom(&[]);
    let res = init(deps.as_mut(), None, None);
    assert_eq!(0, res.messages.len());

    start_sale_future_start(
        deps.as_mut(),
        mock_env(),
        Asset::NativeToken("uusd".to_string()),
    );
    assert_sale_created_future_start(deps.as_ref(), mock_env(), "uusd".to_string(), false);
}

#[test]
fn test_authorized_cw721() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let res = init(
        deps.as_mut(),
        None,
        Some(vec![AndrAddr::from_string(MOCK_CW721_ADDR.to_string())]),
    );
    assert_eq!(0, res.messages.len());

    let current_time = env.block.time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO;
    let hook_msg = Cw721HookMsg::StartSale {
        coin_denom: Asset::NativeToken("uusd".to_string()),
        price: Uint128::new(100),
        // Add one to the current time to have it set in the future
        start_time: Some(Expiry::AtTime(Milliseconds(current_time + 1))),
        duration: None,
        recipient: None,
    };
    let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_owned(),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        msg: encode_binary(&hook_msg).unwrap(),
    });
    let env = mock_env();

    let info = mock_info(MOCK_TOKEN_ADDR, &[]);
    let err = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, err);

    // Now let's set mock cw721 addr as the message sender
    let info = mock_info(MOCK_CW721_ADDR, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    // Add one to the current time to have it set in the future
    let start_time_expiration =
        expiration_from_milliseconds(Milliseconds(current_time + 1)).unwrap();
    assert_eq!(
        TokenSaleState {
            coin_denom: "uusd".to_string(),
            sale_id: 1u128.into(),
            owner: MOCK_TOKEN_OWNER.to_string(),
            token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
            token_address: MOCK_CW721_ADDR.to_owned(),
            status: Status::Open,
            price: Uint128::new(100),
            start_time: start_time_expiration,
            end_time: Expiration::Never {},
            uses_cw20: false,
            recipient: None,
        },
        TOKEN_SALE_STATE.load(deps.as_ref().storage, 1u128).unwrap()
    );
    assert_eq!(
        SaleInfo {
            sale_ids: vec![Uint128::from(1u128)],
            token_address: MOCK_CW721_ADDR.to_owned(),
            token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        },
        sale_infos()
            .load(
                deps.as_ref().storage,
                &(MOCK_UNCLAIMED_TOKEN.to_owned() + MOCK_CW721_ADDR)
            )
            .unwrap()
    );
}

#[test]
fn test_sale_instantiate_future_start_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let res = init(
        deps.as_mut(),
        Some(vec![AndrAddr::from_string(MOCK_CW20_CONTRACT)]),
        None,
    );
    assert_eq!(0, res.messages.len());

    start_sale_future_start(
        deps.as_mut(),
        mock_env(),
        Asset::Cw20Token(AndrAddr::from_string(MOCK_CW20_CONTRACT.to_string())),
    );
    assert_sale_created_future_start(
        deps.as_ref(),
        mock_env(),
        MOCK_CW20_CONTRACT.to_string(),
        true,
    );
}

#[test]
fn test_execute_buy_non_existing_sale() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut(), None, None);
    let env = mock_env();
    let msg = ExecuteMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_string(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    let info = mock_info("buyer", &coins(100, "uusd"));
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::SaleDoesNotExist {}, res.unwrap_err());
}

#[test]
fn test_execute_buy_sale_not_open_already_bought() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut(), None, None);

    start_sale(deps.as_mut(), Asset::NativeToken("uusd".to_string()));
    assert_sale_created(deps.as_ref(), env.clone(), "uusd".to_string(), false);

    let msg = ExecuteMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    // Add one second so that the start_time expires
    env.block.time = env.block.time.plus_seconds(1);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let msg = ExecuteMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::SaleExecuted {})
}

#[test]
fn test_execute_buy_sale_not_open_cancelled() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let _res = init(deps.as_mut(), None, None);

    start_sale(deps.as_mut(), Asset::NativeToken("uusd".to_string()));
    assert_sale_created(deps.as_ref(), env.clone(), "uusd".to_string(), false);

    let msg = ExecuteMsg::CancelSale {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info(MOCK_TOKEN_OWNER, &[]);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let msg = ExecuteMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::SaleCancelled {})
}

#[test]
fn test_execute_buy_token_owner_cannot_buy() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();

    let _res = init(deps.as_mut(), None, None);

    start_sale(deps.as_mut(), Asset::NativeToken("uusd".to_string()));
    assert_sale_created(deps.as_ref(), env.clone(), "uusd".to_string(), false);

    let msg = ExecuteMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    // Add one second so that the start_time expires
    env.block.time = env.block.time.plus_seconds(1);

    let info = mock_info(MOCK_TOKEN_OWNER, &coins(100, "uusd".to_string()));
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::TokenOwnerCannotBuy {}, res.unwrap_err());
}

#[test]
fn test_execute_buy_token_owner_cannot_buy_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();

    let _res = init(
        deps.as_mut(),
        Some(vec![AndrAddr::from_string(MOCK_CW20_CONTRACT)]),
        None,
    );

    let uses_cw20 = true;
    start_sale(
        deps.as_mut(),
        Asset::Cw20Token(AndrAddr::from_string(MOCK_CW20_CONTRACT.to_string())),
    );
    assert_sale_created(
        deps.as_ref(),
        env.clone(),
        MOCK_CW20_CONTRACT.to_string(),
        uses_cw20,
    );

    let hook_msg = Cw20HookMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_string(),
        amount: Uint128::new(100),
        msg: encode_binary(&hook_msg).unwrap(),
    });

    let info = mock_info(MOCK_CW20_CONTRACT, &[]);

    // Add one second so that the start_time expires
    env.block.time = env.block.time.plus_seconds(1);

    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::TokenOwnerCannotBuy {}, res.unwrap_err());
}

#[test]
fn test_execute_buy_invalid_coins_sent() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();

    let _res = init(deps.as_mut(), None, None);

    start_sale(deps.as_mut(), Asset::NativeToken("uusd".to_string()));
    assert_sale_created(deps.as_ref(), env.clone(), "uusd".to_string(), false);

    let error = ContractError::InvalidFunds {
        msg: "One coin should be sent.".to_string(),
    };
    let msg = ExecuteMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_string(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    // No coins sent
    let info = mock_info("sender", &[]);
    // Add one second so that the start_time expires
    env.block.time = env.block.time.plus_seconds(1);
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert_eq!(error, res.unwrap_err());

    // Multiple coins sent
    let info = mock_info("sender", &[coin(100, "uusd"), coin(100, "uluna")]);
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert_eq!(error, res.unwrap_err());

    // Invalid denom sent
    let info = mock_info("sender", &[coin(100, "uluna")]);
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert_eq!(
        ContractError::InvalidFunds {
            msg: "No uusd assets are provided to sale".to_string(),
        },
        res.unwrap_err()
    );

    // Correct denom but empty
    let info = mock_info("sender", &[coin(0, "uusd")]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert!(matches!(res, ContractError::InvalidFunds { .. }));
}

#[test]
fn test_execute_buy_invalid_coins_sent_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();

    let _res = init(
        deps.as_mut(),
        Some(vec![AndrAddr::from_string(MOCK_CW20_CONTRACT)]),
        None,
    );

    let uses_cw20 = true;
    start_sale(
        deps.as_mut(),
        Asset::Cw20Token(AndrAddr::from_string(MOCK_CW20_CONTRACT.to_string())),
    );
    assert_sale_created(
        deps.as_ref(),
        env.clone(),
        MOCK_CW20_CONTRACT.to_string(),
        uses_cw20,
    );

    let hook_msg = Cw20HookMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    // No coins sent
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "buyer".to_string(),
        amount: Uint128::zero(),
        msg: encode_binary(&hook_msg).unwrap(),
    });

    let info = mock_info(MOCK_CW20_CONTRACT, &[]);

    // Add one second so that the start_time expires
    env.block.time = env.block.time.plus_seconds(1);
    let res = execute(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(
        ContractError::InvalidFunds {
            msg: "Cannot send a 0 amount".to_string(),
        },
        res.unwrap_err()
    );

    let hook_msg = Cw20HookMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "buyer".to_string(),
        amount: Uint128::new(100),
        msg: encode_binary(&hook_msg).unwrap(),
    });
    // Invalid denom sent
    let info = mock_info("invalid_cw20", &[]);

    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
}

#[test]
fn test_execute_buy_works() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();

    let _res = init(
        deps.as_mut(),
        Some(vec![AndrAddr::from_string(MOCK_CW20_CONTRACT)]),
        None,
    );

    start_sale(deps.as_mut(), Asset::NativeToken("uusd".to_string()));
    assert_sale_created(deps.as_ref(), env.clone(), "uusd".to_string(), false);

    let msg = ExecuteMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("someone", &coins(100, "uusd".to_string()));
    // Add one second so that the start_time expires
    env.block.time = env.block.time.plus_seconds(1);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
}

#[test]
fn test_execute_buy_works_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();

    let _res = init(
        deps.as_mut(),
        Some(vec![AndrAddr::from_string(MOCK_CW20_CONTRACT)]),
        None,
    );

    let uses_cw20 = true;
    start_sale(
        deps.as_mut(),
        Asset::Cw20Token(AndrAddr::from_string(MOCK_CW20_CONTRACT.to_string())),
    );
    assert_sale_created(
        deps.as_ref(),
        env.clone(),
        MOCK_CW20_CONTRACT.to_string(),
        uses_cw20,
    );

    let hook_msg = Cw20HookMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "someone".to_string(),
        amount: Uint128::new(100),
        msg: encode_binary(&hook_msg).unwrap(),
    });

    let info = mock_info(MOCK_CW20_CONTRACT, &[]);
    // Add one second so that the start_time expires
    env.block.time = env.block.time.plus_seconds(1);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
}

#[test]
fn test_execute_buy_future_start() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let _res = init(deps.as_mut(), None, None);

    start_sale_future_start(
        deps.as_mut(),
        mock_env(),
        Asset::NativeToken("uusd".to_string()),
    );
    assert_sale_created_future_start(deps.as_ref(), mock_env(), "uusd".to_string(), false);

    let msg = ExecuteMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("someone", &coins(100, "uusd".to_string()));
    // The start time is ahead of the current block time, so it should return a Sale Not Started error.
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::SaleNotOpen {})
}

#[test]
fn test_execute_buy_sale_expired() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();

    let _res = init(deps.as_mut(), None, None);

    start_sale_future_start_with_duration(deps.as_mut(), mock_env());

    let msg = ExecuteMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("someone", &coins(100, "uusd".to_string()));
    // Forward block time so that the end time expires
    env.block.time = env.block.time.plus_days(100);

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::SaleExpired {})
}

#[test]
fn test_execute_update_sale_unauthorized() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let _res = init(deps.as_mut(), None, None);

    start_sale(deps.as_mut(), Asset::NativeToken("uusd".to_string()));
    assert_sale_created(deps.as_ref(), env.clone(), "uusd".to_string(), false);

    let msg = ExecuteMsg::UpdateSale {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
        price: Uint128::new(11),
        coin_denom: Asset::NativeToken("juno".to_string()),
        recipient: None,
    };

    let info = mock_info("someone", &[]);
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {})
}

#[test]
fn test_execute_update_sale_invalid_price() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let _res = init(deps.as_mut(), None, None);

    start_sale(deps.as_mut(), Asset::NativeToken("uusd".to_string()));
    assert_sale_created(deps.as_ref(), env.clone(), "uusd".to_string(), false);

    let msg = ExecuteMsg::UpdateSale {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
        price: Uint128::zero(),
        coin_denom: Asset::NativeToken("juno".to_string()),
        recipient: None,
    };

    let info = mock_info("owner", &[]);
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidZeroAmount {})
}

#[test]
fn test_execute_start_sale_invalid_price() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut(), None, None);

    let hook_msg = Cw721HookMsg::StartSale {
        coin_denom: Asset::NativeToken("uusd".to_string()),
        price: Uint128::zero(),
        start_time: None,
        duration: None,
        recipient: None,
    };
    let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_owned(),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        msg: encode_binary(&hook_msg).unwrap(),
    });
    let env = mock_env();

    let info = mock_info(MOCK_TOKEN_ADDR, &[]);
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidZeroAmount {})
}

#[test]
fn test_execute_buy_with_tax_and_royalty_insufficient_funds() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut(), None, None);

    start_sale(deps.as_mut(), Asset::NativeToken("uusd".to_string()));
    assert_sale_created(deps.as_ref(), mock_env(), "uusd".to_string(), false);

    let rate = Rate::Local(LocalRate {
        rate_type: LocalRateType::Additive,
        recipient: Recipient {
            address: AndrAddr::from_string("tax_recipient".to_string()),
            msg: None,
            ibc_recovery_address: None,
        },
        value: LocalRateValue::Percent(PercentRate {
            percent: Decimal::percent(50),
        }),
        description: None,
    });

    // Set rates
    ADOContract::default()
        .set_rates(deps.as_mut().storage, "Buy", rate)
        .unwrap();

    let msg = ExecuteMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    let mut env = mock_env();
    // Add one second so that the start_time expires
    env.block.time = env.block.time.plus_seconds(1);
    let info = mock_info("someone", &coins(100, "uusd".to_string()));
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidFunds {
            msg: "Invalid funds provided, expected: 150, received: 100".to_string()
        }
    );
}

#[test]
fn test_execute_buy_with_tax_and_royalty_insufficient_funds_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(
        deps.as_mut(),
        Some(vec![AndrAddr::from_string(MOCK_CW20_CONTRACT)]),
        None,
    );

    let uses_cw20 = true;
    start_sale(
        deps.as_mut(),
        Asset::Cw20Token(AndrAddr::from_string(MOCK_CW20_CONTRACT.to_string())),
    );
    assert_sale_created(
        deps.as_ref(),
        mock_env(),
        MOCK_CW20_CONTRACT.to_string(),
        uses_cw20,
    );

    let rate = Rate::Local(LocalRate {
        rate_type: LocalRateType::Additive,
        recipient: Recipient {
            address: AndrAddr::from_string("tax_recipient".to_string()),
            msg: None,
            ibc_recovery_address: None,
        },
        value: LocalRateValue::Percent(PercentRate {
            percent: Decimal::percent(50),
        }),
        description: None,
    });

    // Set rates
    ADOContract::default()
        .set_rates(deps.as_mut().storage, "Buy", rate)
        .unwrap();

    let hook_msg = Cw20HookMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "someone".to_string(),
        amount: Uint128::new(100),
        msg: encode_binary(&hook_msg).unwrap(),
    });

    let info = mock_info(MOCK_CW20_CONTRACT, &[]);

    let mut env = mock_env();
    // Add one second so that the start_time expires
    env.block.time = env.block.time.plus_seconds(1);
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidFunds {
            msg: "Invalid funds provided, expected: 150, received: 100".to_string()
        }
    );
}

#[test]
fn execute_buy_with_tax_and_royalty_too_many_funds() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut(), None, None);

    start_sale(deps.as_mut(), Asset::NativeToken("uusd".to_string()));
    assert_sale_created(deps.as_ref(), mock_env(), "uusd".to_string(), false);

    let msg = ExecuteMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    let mut env = mock_env();
    // Add one second so that the start_time expires
    env.block.time = env.block.time.plus_seconds(1);

    let info = mock_info("someone", &[coin(200, "uusd"), coin(100, "uandr")]);
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert!(matches!(err, ContractError::InvalidFunds { .. }));
}

// TODO having both tax and royalty is currently unsupported
#[test]
fn test_execute_buy_with_tax_and_royalty_works() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut(), None, None);

    start_sale(deps.as_mut(), Asset::NativeToken("uusd".to_string()));
    assert_sale_created(deps.as_ref(), mock_env(), "uusd".to_string(), false);

    let msg = ExecuteMsg::Buy {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let rate = Rate::Local(LocalRate {
        rate_type: LocalRateType::Additive,
        recipient: Recipient {
            address: AndrAddr::from_string("tax_recipient".to_string()),
            msg: None,
            ibc_recovery_address: None,
        },
        value: LocalRateValue::Percent(PercentRate {
            percent: Decimal::percent(50),
        }),
        description: None,
    });

    // Set rates
    ADOContract::default()
        .set_rates(deps.as_mut().storage, "Buy", rate)
        .unwrap();

    let info = mock_info("someone", &coins(150, "uusd".to_string()));
    let mut env = mock_env();
    // Add one second so that the start_time expires
    env.block.time = env.block.time.plus_seconds(1);

    let res = execute(deps.as_mut(), env, info.clone(), msg).unwrap();
    let expected: Vec<SubMsg<_>> = vec![
        // SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
        //     to_address: "royalty_recipient".to_string(),
        //     amount: vec![coin(10, "uusd")],
        // })),

        // SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
        //     to_address: "owner".to_string(),
        //     amount: vec![coin(90, "uusd")],
        // })),
        SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: "tax_recipient".to_string(),
            amount: vec![coin(50, "uusd")],
        })),
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_TOKEN_ADDR.to_string(),
            msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                recipient: info.sender.to_string(),
                token_id: MOCK_UNCLAIMED_TOKEN.to_string(),
            })
            .unwrap(),
            funds: vec![],
        })),
        SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: "owner".to_string(),
            amount: vec![coin(100, "uusd")],
        })),
    ];
    assert_eq!(res.messages, expected)
}
#[test]
fn test_execute_authorize_cw20_contract() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut(), None, None);

    // Test unauthorized attempt
    let unauthorized_info = mock_info("unauthorized", &[]);
    let unauthorized_msg = ExecuteMsg::AuthorizeContract {
        action: PermissionAction::SendCw20,
        addr: AndrAddr::from_string("cw20_contract"),
        expiration: None,
    };
    let unauthorized_result = execute(
        deps.as_mut(),
        mock_env(),
        unauthorized_info,
        unauthorized_msg,
    );
    assert_eq!(
        unauthorized_result.unwrap_err(),
        ContractError::Unauthorized {}
    );

    // Test successful authorization without expiration
    let owner_info = mock_info("owner", &[]);
    let msg = ExecuteMsg::AuthorizeContract {
        action: PermissionAction::SendCw20,
        addr: AndrAddr::from_string("cw20_contract"),
        expiration: None,
    };
    let result = execute(deps.as_mut(), mock_env(), owner_info, msg).unwrap();

    assert_eq!(
        result.attributes,
        vec![
            attr("action", "authorize_contract"),
            attr("address", "cw20_contract"),
            attr("permission", "whitelisted"),
        ]
    );

    // Verify the permission was set correctly
    let permission =
        ADOContract::get_permission(deps.as_ref().storage, SEND_CW20_ACTION, "cw20_contract")
            .unwrap();
    assert_eq!(
        permission,
        Some(Permission::Local(LocalPermission::Whitelisted(None)))
    );

    // Test successful authorization with expiration
    let owner_info = mock_info("owner", &[]);
    let expiration = Expiry::FromNow(Milliseconds(10000));
    let msg = ExecuteMsg::AuthorizeContract {
        action: PermissionAction::SendCw20,
        addr: AndrAddr::from_string("cw20_contract_with_expiry"),
        expiration: Some(expiration.clone()),
    };
    let result = execute(deps.as_mut(), mock_env(), owner_info, msg).unwrap();

    assert_eq!(
        result.attributes,
        vec![
            attr("action", "authorize_contract"),
            attr("address", "cw20_contract_with_expiry"),
            attr("permission", format!("whitelisted:{}", expiration)),
        ]
    );

    // Verify the permission was set correctly with expiration
    let permission = ADOContract::get_permission(
        deps.as_ref().storage,
        SEND_CW20_ACTION,
        "cw20_contract_with_expiry",
    )
    .unwrap();
    assert_eq!(
        permission,
        Some(Permission::Local(LocalPermission::Whitelisted(Some(
            expiration
        ))))
    );
}

#[test]
fn test_execute_deauthorize_cw20_contract() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut(), None, None);

    // First, authorize a CW20 contract
    let owner_info = mock_info("owner", &[]);
    let msg = ExecuteMsg::AuthorizeContract {
        action: PermissionAction::SendCw20,
        addr: AndrAddr::from_string("cw20_contract"),
        expiration: None,
    };
    let _res = execute(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

    // Verify the permission was set
    let permission =
        ADOContract::get_permission(deps.as_ref().storage, SEND_CW20_ACTION, "cw20_contract")
            .unwrap();
    assert_eq!(
        permission,
        Some(Permission::Local(LocalPermission::Whitelisted(None)))
    );

    // Now deauthorize the CW20 contract
    let msg = ExecuteMsg::DeauthorizeContract {
        action: PermissionAction::SendCw20,
        addr: AndrAddr::from_string("cw20_contract"),
    };
    let res = execute(deps.as_mut(), mock_env(), owner_info, msg).unwrap();

    // Check the response
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "deauthorize_contract"),
            attr("address", "cw20_contract"),
            attr("deauthorized_action", SEND_CW20_ACTION),
        ]
    );

    // Verify the permission was removed
    let permission =
        ADOContract::get_permission(deps.as_ref().storage, SEND_CW20_ACTION, "cw20_contract")
            .unwrap();
    assert_eq!(permission, None);

    // Test deauthorization by non-owner (should fail)
    let non_owner_info = mock_info("not_owner", &[]);
    let msg = ExecuteMsg::DeauthorizeContract {
        action: PermissionAction::SendCw20,
        addr: AndrAddr::from_string("cw20_contract"),
    };
    let err = execute(deps.as_mut(), mock_env(), non_owner_info, msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_query_authorized_addresses() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(
        deps.as_mut(),
        Some(vec![
            AndrAddr::from_string("cw20_contract1"),
            AndrAddr::from_string("cw20_contract2"),
        ]),
        Some(vec![
            AndrAddr::from_string("nft_contract1"),
            AndrAddr::from_string("nft_contract2"),
        ]),
    );

    // Query authorized addresses for CW20 action
    let cw20_query = QueryMsg::AuthorizedAddresses {
        action: PermissionAction::SendCw20,
        start_after: None,
        limit: None,
        order_by: None,
    };
    let cw20_res: AuthorizedAddressesResponse =
        from_json(query(deps.as_ref(), mock_env(), cw20_query).unwrap()).unwrap();
    assert_eq!(
        cw20_res.addresses,
        vec!["cw20_contract1".to_string(), "cw20_contract2".to_string()]
    );

    // Query authorized addresses for NFT action
    let nft_query = QueryMsg::AuthorizedAddresses {
        action: PermissionAction::SendNft,
        start_after: None,
        limit: None,
        order_by: None,
    };
    let nft_res: AuthorizedAddressesResponse =
        from_json(query(deps.as_ref(), mock_env(), nft_query).unwrap()).unwrap();
    assert_eq!(
        nft_res.addresses,
        vec!["nft_contract1".to_string(), "nft_contract2".to_string()]
    );
}
#[test]
fn test_authorize_token_contract() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut(), None, None);

    let owner_info = mock_info("owner", &[]);
    let token_address = AndrAddr::from_string("nft_contract");
    let expiration = Expiry::FromNow(Milliseconds(100));

    // Test successful authorization
    let msg = ExecuteMsg::AuthorizeContract {
        action: PermissionAction::SendNft,
        addr: token_address.clone(),
        expiration: Some(expiration.clone()),
    };
    let res = execute(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "authorize_contract"),
            attr("address", "nft_contract"),
            attr("permission", format!("whitelisted:{}", expiration)),
        ]
    );

    // Test unauthorized attempt
    let non_owner_info = mock_info("non_owner", &[]);
    let msg = ExecuteMsg::AuthorizeContract {
        action: PermissionAction::SendNft,
        addr: token_address.clone(),
        expiration: None,
    };
    let err = execute(deps.as_mut(), mock_env(), non_owner_info, msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // Query to verify authorization
    let query_msg = QueryMsg::AuthorizedAddresses {
        action: PermissionAction::SendNft,
        start_after: None,
        limit: None,
        order_by: None,
    };
    let res: AuthorizedAddressesResponse =
        from_json(query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
    assert_eq!(res.addresses, vec!["nft_contract".to_string()]);
}

#[test]
fn test_deauthorize_token_contract() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(
        deps.as_mut(),
        None,
        Some(vec![AndrAddr::from_string("nft_contract")]),
    );

    let owner_info = mock_info("owner", &[]);
    let token_address = AndrAddr::from_string("nft_contract");

    // Test successful deauthorization
    let msg = ExecuteMsg::DeauthorizeContract {
        action: PermissionAction::SendNft,
        addr: token_address.clone(),
    };
    let res = execute(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "deauthorize_contract"),
            attr("address", "nft_contract"),
            attr("deauthorized_action", SEND_NFT_ACTION),
        ]
    );

    // Test unauthorized attempt
    let non_owner_info = mock_info("non_owner", &[]);
    let msg = ExecuteMsg::DeauthorizeContract {
        action: PermissionAction::SendNft,
        addr: token_address.clone(),
    };
    let err = execute(deps.as_mut(), mock_env(), non_owner_info, msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // Query to verify deauthorization
    let query_msg = QueryMsg::AuthorizedAddresses {
        action: PermissionAction::SendNft,
        start_after: None,
        limit: None,
        order_by: None,
    };
    let res: AuthorizedAddressesResponse =
        from_json(query(deps.as_ref(), mock_env(), query_msg).unwrap()).unwrap();
    assert!(res.addresses.is_empty());
}
