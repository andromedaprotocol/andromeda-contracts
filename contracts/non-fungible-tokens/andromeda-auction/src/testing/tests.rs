use crate::{
    contract::{execute, instantiate, query},
    state::{auction_infos, TOKEN_AUCTION_STATE},
    testing::mock_querier::{
        mock_dependencies_custom, MOCK_TOKEN_ADDR, MOCK_TOKEN_OWNER, MOCK_UNCLAIMED_TOKEN,
    },
};

use andromeda_non_fungible_tokens::{
    auction::{
        AuctionInfo, AuctionStateResponse, Cw20HookMsg, Cw721HookMsg, ExecuteMsg, InstantiateMsg,
        QueryMsg, TokenAuctionState,
    },
    cw721::ExecuteMsg as Cw721ExecuteMsg,
};
use andromeda_std::{
    ado_base::{
        modules::Module,
        rates::{LocalRate, LocalRateType, LocalRateValue, PercentRate, Rate},
    },
    ado_contract::ADOContract,
    amp::AndrAddr,
    common::{
        denom::Asset,
        encode_binary,
        expiration::{Expiry, MILLISECONDS_TO_NANOSECONDS_RATIO},
        Milliseconds,
    },
    error::ContractError,
    testing::mock_querier::MOCK_KERNEL_CONTRACT,
};
use andromeda_std::{amp::Recipient, testing::mock_querier::MOCK_CW20_CONTRACT};
use cosmwasm_std::{
    attr, coin, coins, from_json,
    testing::{mock_dependencies, mock_env, mock_info},
    Addr, BankMsg, CosmosMsg, Decimal, Deps, DepsMut, Env, Response, Timestamp, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw721::Cw721ReceiveMsg;
use cw_utils::Expiration;

fn init(deps: DepsMut) -> Response {
    let msg = InstantiateMsg {
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        authorized_token_addresses: Some(vec![AndrAddr::from_string(MOCK_TOKEN_ADDR)]),
        authorized_cw20_addresses: None,
    };

    let info = mock_info("owner", &[]);
    instantiate(deps, mock_env(), info, msg).unwrap()
}

fn init_cw20(deps: DepsMut, _modules: Option<Vec<Module>>) -> Response {
    let msg = InstantiateMsg {
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        authorized_token_addresses: Some(vec![AndrAddr::from_string(MOCK_TOKEN_ADDR)]),
        authorized_cw20_addresses: Some(vec![AndrAddr::from_string(MOCK_CW20_CONTRACT)]),
    };

    let info = mock_info("owner", &[]);
    instantiate(deps, mock_env(), info, msg).unwrap()
}

fn query_latest_auction_state_helper(deps: Deps, env: Env) -> AuctionStateResponse {
    let query_msg = QueryMsg::LatestAuctionState {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_owned(),
    };
    from_json(query(deps, env, query_msg).unwrap()).unwrap()
}

fn current_time() -> u64 {
    mock_env().block.time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO
}

fn start_auction(
    deps: DepsMut,
    whitelist: Option<Vec<Addr>>,
    min_bid: Option<Uint128>,
    min_raise: Option<Uint128>,
    buy_now_price: Option<Uint128>,
) {
    let hook_msg = Cw721HookMsg::StartAuction {
        start_time: None,
        end_time: Expiry::FromNow(Milliseconds(20_000_000)),
        coin_denom: Asset::NativeToken("uusd".to_string()),
        whitelist,
        min_bid,
        min_raise,
        recipient: None,
        buy_now_price,
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

fn start_auction_cw20(
    deps: DepsMut,
    whitelist: Option<Vec<Addr>>,
    min_bid: Option<Uint128>,
    min_raise: Option<Uint128>,
    buy_now_price: Option<Uint128>,
) {
    let hook_msg = Cw721HookMsg::StartAuction {
        start_time: None,
        end_time: Expiry::FromNow(Milliseconds(20_000_000)),
        coin_denom: Asset::Cw20Token(AndrAddr::from_string(MOCK_CW20_CONTRACT.to_string())),
        whitelist,
        min_bid,
        min_raise,
        recipient: None,
        buy_now_price,
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

fn assert_auction_created(
    deps: Deps,
    whitelist: Option<Vec<Addr>>,
    min_bid: Option<Uint128>,
    min_raise: Option<Uint128>,
    buy_now_price: Option<Uint128>,
) {
    let current_time = mock_env().block.time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO;
    let duration = 20_000_000;
    assert_eq!(
        TokenAuctionState {
            start_time: Expiration::AtTime(Timestamp::from_nanos((current_time + 1) * 1_000_000)),
            end_time: Expiration::AtTime(Timestamp::from_nanos(
                (current_time + duration) * 1_000_000
            )),
            high_bidder_addr: Addr::unchecked(""),
            high_bidder_amount: Uint128::zero(),
            buy_now_price,
            coin_denom: "uusd".to_string(),
            uses_cw20: false,
            auction_id: 1u128.into(),
            owner: MOCK_TOKEN_OWNER.to_string(),
            token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
            token_address: MOCK_TOKEN_ADDR.to_owned(),
            is_cancelled: false,
            is_bought: false,
            min_bid,
            min_raise,
            whitelist,
            recipient: None
        },
        TOKEN_AUCTION_STATE.load(deps.storage, 1u128).unwrap()
    );

    assert_eq!(
        AuctionInfo {
            auction_ids: vec![Uint128::from(1u128)],
            token_address: MOCK_TOKEN_ADDR.to_owned(),
            token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        },
        auction_infos()
            .load(
                deps.storage,
                &(MOCK_UNCLAIMED_TOKEN.to_owned() + MOCK_TOKEN_ADDR)
            )
            .unwrap()
    );
}

fn assert_auction_created_cw20(
    deps: Deps,
    whitelist: Option<Vec<Addr>>,
    min_bid: Option<Uint128>,
    min_raise: Option<Uint128>,
) {
    let current_time = mock_env().block.time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO;
    let duration = 20_000_000;
    assert_eq!(
        TokenAuctionState {
            start_time: Expiration::AtTime(Timestamp::from_nanos((current_time + 1) * 1_000_000)),
            end_time: Expiration::AtTime(Timestamp::from_nanos(
                (current_time + duration) * 1_000_000
            )),
            high_bidder_addr: Addr::unchecked(""),
            high_bidder_amount: Uint128::zero(),
            coin_denom: MOCK_CW20_CONTRACT.to_string(),
            buy_now_price: None,
            uses_cw20: true,
            auction_id: 1u128.into(),
            owner: MOCK_TOKEN_OWNER.to_string(),
            token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
            token_address: MOCK_TOKEN_ADDR.to_owned(),
            is_cancelled: false,
            is_bought: false,
            min_bid,
            min_raise,
            whitelist,
            recipient: None
        },
        TOKEN_AUCTION_STATE.load(deps.storage, 1u128).unwrap()
    );

    assert_eq!(
        AuctionInfo {
            auction_ids: vec![Uint128::from(1u128)],
            token_address: MOCK_TOKEN_ADDR.to_owned(),
            token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        },
        auction_infos()
            .load(
                deps.storage,
                &(MOCK_UNCLAIMED_TOKEN.to_owned() + MOCK_TOKEN_ADDR)
            )
            .unwrap()
    );
}

#[test]
fn test_auction_instantiate() {
    let mut deps = mock_dependencies();
    let res = init(deps.as_mut());
    assert_eq!(0, res.messages.len());
}

#[test]
fn test_auction_instantiate_cw20() {
    let mut deps = mock_dependencies();
    let res = init_cw20(deps.as_mut(), None);
    assert_eq!(0, res.messages.len());
}

#[test]
fn test_execute_place_bid_non_existing_auction() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let _res = init(deps.as_mut());

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_string(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    let info = mock_info("bidder", &coins(100, "uusd"));
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::AuctionDoesNotExist {}, res.unwrap_err());
}

#[test]
fn execute_place_bid_auction_not_started() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);
    assert_auction_created(deps.as_ref(), None, None, None, None);

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    env.block.time = Timestamp::from_seconds(50u64);

    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::AuctionNotStarted {}, res.unwrap_err());
}

#[test]
fn execute_place_bid_auction_ended() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);
    assert_auction_created(deps.as_ref(), None, None, None, None);

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    env.block.time = env.block.time.plus_days(1);

    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::AuctionEnded {}, res.unwrap_err());
}

#[test]
fn execute_place_bid_token_owner_cannot_bid() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);
    assert_auction_created(deps.as_ref(), None, None, None, None);

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    env.block.time = env.block.time.plus_seconds(1);
    let info = mock_info(MOCK_TOKEN_OWNER, &coins(100, "uusd".to_string()));
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::TokenOwnerCannotBid {}, res.unwrap_err());
}

#[test]
fn execute_place_bid_min_raise() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, Some(Uint128::new(10)), None);
    assert_auction_created(deps.as_ref(), None, None, Some(Uint128::new(10)), None);

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    env.block.time = env.block.time.plus_seconds(1);

    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    let _res = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap();

    // Difference is less than 10, which is the minimum raise
    let info = mock_info("other_sender", &coins(109, "uusd".to_string()));
    let err = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap_err();
    assert_eq!(err, ContractError::MinRaiseUnmet {});

    // Difference is 10, which meets the minimum raise
    let info = mock_info("other_sender", &coins(110, "uusd".to_string()));
    let _res = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap();

    // Difference exceeds minimum raise
    let info = mock_info("other_other_sender", &coins(200, "uusd".to_string()));
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
}

#[test]
fn execute_min_bid_greater_than_buy_now() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let _res = init(deps.as_mut());
    let hook_msg = Cw721HookMsg::StartAuction {
        start_time: None,
        end_time: Expiry::FromNow(Milliseconds(20_000_000)),
        coin_denom: Asset::NativeToken("uusd".to_string()),
        whitelist: None,
        min_bid: Some(Uint128::new(100)),
        min_raise: None,
        recipient: None,
        buy_now_price: Some(Uint128::one()),
    };
    let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_owned(),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        msg: encode_binary(&hook_msg).unwrap(),
    });
    let info = mock_info(MOCK_TOKEN_ADDR, &[]);
    let err = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidMinBid {
            msg: Some("buy_now_price must be greater than the min_bid".to_string())
        }
    )
}

#[test]
fn execute_place_bid_whitelist() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(
        deps.as_mut(),
        Some(vec![Addr::unchecked("sender")]),
        None,
        None,
        None,
    );
    assert_auction_created(
        deps.as_ref(),
        Some(vec![Addr::unchecked("sender")]),
        None,
        None,
        None,
    );

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("not_sender", &coins(100, "uusd".to_string()));
    env.block.time = env.block.time.plus_seconds(1);
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());

    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
}

#[test]
fn execute_place_bid_whitelist_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init_cw20(deps.as_mut(), None);

    start_auction_cw20(
        deps.as_mut(),
        Some(vec![Addr::unchecked("sender")]),
        None,
        None,
        None,
    );
    assert_auction_created_cw20(
        deps.as_ref(),
        Some(vec![Addr::unchecked("sender")]),
        None,
        None,
    );

    let hook_msg = Cw20HookMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "sender".to_string(),
        amount: Uint128::new(100),
        msg: encode_binary(&hook_msg).unwrap(),
    });

    let invalid_asset = "invalid_asset";
    let info = mock_info(invalid_asset, &[]);
    env.block.time = env.block.time.plus_seconds(1);
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert_eq!(
        ContractError::InvalidAsset {
            asset: invalid_asset.to_string()
        },
        res.unwrap_err()
    );

    let info = mock_info(MOCK_CW20_CONTRACT, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
}

#[test]
fn execute_place_bid_highest_bidder_cannot_outbid() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);
    assert_auction_created(deps.as_ref(), None, None, None, None);

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    env.block.time = env.block.time.plus_seconds(1);
    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    let _res = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap();
    env.block.time = env.block.time.plus_seconds(2);
    let info = mock_info("sender", &coins(200, "uusd".to_string()));
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(
        ContractError::HighestBidderCannotOutBid {},
        res.unwrap_err()
    );
}

#[test]
fn execute_place_bid_bid_smaller_than_highest_bid() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);
    assert_auction_created(deps.as_ref(), None, None, None, None);

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    env.block.time = env.block.time.plus_seconds(1);
    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    let _res = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap();

    env.block.time = env.block.time.plus_seconds(2);
    let info = mock_info("other", &coins(50, "uusd".to_string()));
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::BidSmallerThanHighestBid {}, res.unwrap_err());
}

#[test]
fn execute_place_bid_invalid_coins_sent() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);
    assert_auction_created(deps.as_ref(), None, None, None, None);

    let error = ContractError::InvalidFunds {
        msg: "One coin should be sent.".to_string(),
    };
    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_string(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    env.block.time = env.block.time.plus_seconds(1);

    // No coins sent
    let info = mock_info("sender", &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert_eq!(error, res.unwrap_err());

    // Multiple coins sent
    let info = mock_info("sender", &[coin(100, "uusd"), coin(100, "uluna")]);
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert_eq!(error, res.unwrap_err());

    let error = ContractError::InvalidFunds {
        msg: "Invalid denomination: expected uusd, got uluna".to_string(),
    };

    // Invalid denom sent
    let info = mock_info("sender", &[coin(100, "uluna")]);
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert_eq!(error, res.unwrap_err());

    let error = ContractError::InvalidFunds {
        msg: "Amount of funds should be greater than 0".to_string(),
    };

    // Correct denom but empty
    let info = mock_info("sender", &[coin(0, "uusd")]);
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(error, res.unwrap_err());
}

#[test]
fn execute_place_bid_multiple_bids() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);
    assert_auction_created(deps.as_ref(), None, None, None, None);

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    env.block.time = env.block.time.plus_seconds(1);
    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

    assert_eq!(
        Response::new().add_attributes(vec![
            attr("action", "bid"),
            attr("token_id", MOCK_UNCLAIMED_TOKEN),
            attr("bidder", info.sender),
            attr("amount", "100"),
        ]),
        res
    );
    let mut expected_response = AuctionStateResponse {
        start_time: Expiration::AtTime(Timestamp::from_nanos(1571797419880000000)),
        end_time: Expiration::AtTime(Timestamp::from_nanos(1571817419879000000)),
        high_bidder_addr: "sender".to_string(),
        high_bidder_amount: Uint128::from(100u128),
        auction_id: Uint128::from(1u128),
        coin_denom: "uusd".to_string(),
        uses_cw20: false,
        is_cancelled: false,
        min_bid: None,
        min_raise: None,
        whitelist: None,
        owner: "owner".to_string(),
        recipient: None,
    };

    let res = query_latest_auction_state_helper(deps.as_ref(), env.clone());
    assert_eq!(expected_response, res);

    env.block.time = env.block.time.plus_seconds(2);
    let info = mock_info("other", &coins(200, "uusd".to_string()));
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        Response::new()
            .add_message(CosmosMsg::Bank(BankMsg::Send {
                to_address: "sender".to_string(),
                amount: coins(100, "uusd")
            }))
            .add_attributes(vec![
                attr("action", "bid"),
                attr("token_id", MOCK_UNCLAIMED_TOKEN),
                attr("bidder", info.sender),
                attr("amount", "200"),
            ]),
        res
    );

    expected_response.high_bidder_addr = "other".to_string();
    expected_response.high_bidder_amount = Uint128::from(200u128);
    let res = query_latest_auction_state_helper(deps.as_ref(), env.clone());
    assert_eq!(expected_response, res);

    env.block.time = env.block.time.plus_seconds(3);
    let info = mock_info("sender", &coins(250, "uusd".to_string()));
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    assert_eq!(
        Response::new()
            .add_message(CosmosMsg::Bank(BankMsg::Send {
                to_address: "other".to_string(),
                amount: coins(200, "uusd")
            }))
            .add_attributes(vec![
                attr("action", "bid"),
                attr("token_id", MOCK_UNCLAIMED_TOKEN),
                attr("bidder", info.sender),
                attr("amount", "250"),
            ]),
        res
    );

    expected_response.high_bidder_addr = "sender".to_string();
    expected_response.high_bidder_amount = Uint128::from(250u128);
    let res = query_latest_auction_state_helper(deps.as_ref(), env);
    assert_eq!(expected_response, res);
}

#[test]
fn execute_place_bid_auction_cancelled() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);
    assert_auction_created(deps.as_ref(), None, None, None, None);

    let msg = ExecuteMsg::CancelAuction {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info(MOCK_TOKEN_OWNER, &[]);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::AuctionCancelled {}, res.unwrap_err());
}

#[test]
fn test_execute_start_auction() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut());
    start_auction(deps.as_mut(), None, None, None, None);
    assert_auction_created(deps.as_ref(), None, None, None, None);
}

#[test]
fn test_execute_start_auction_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init_cw20(deps.as_mut(), None);
    start_auction_cw20(deps.as_mut(), None, None, None, None);
    assert_auction_created_cw20(deps.as_ref(), None, None, None);
}

// #[test]
// fn execute_start_auction_with_block_height() {
//     let mut deps = mock_dependencies_custom(&[]);
//     let env = mock_env();
//     let info = mock_info("owner", &[]);
//     let msg = InstantiateMsg {  kernel_address: None };
//     let _res = instantiate(deps.as_mut(), env, &info, msg).unwrap();

//     let hook_msg = Cw721HookMsg::StartAuction {
//         start_time: Expiration::AtHeight(100),
//         end_time: Expiration::AtHeight(200),
//         coin_denom: "uusd".to_string(),
//         whitelist: None,
//         min_bid: None,
//     };
//     let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
//         sender: MOCK_TOKEN_OWNER.to_owned(),
//         token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
//         msg: encode_binary(&hook_msg).unwrap(),
//     });
//     let mut env = mock_env();
//     env.block.height = 0;

//     let info = mock_info(MOCK_TOKEN_ADDR, &[]);
//     let res = execute(deps.as_mut(), env, &info, msg).unwrap();

//     assert_eq!(
//         res,
//         Response::new().add_attributes(vec![
//             attr("action", "start_auction"),
//             attr("start_time", "expiration height: 100"),
//             attr("end_time", "expiration height: 200"),
//             attr("coin_denom", "uusd"),
//             attr("auction_id", "1"),
//             attr("whitelist", "None"),
//         ]),
//     );
// }

// #[test]
// fn execute_start_auction_with_mismatched_expirations() {
//     let mut deps = mock_dependencies_custom(&[]);
//     let env = mock_env();
//     let info = mock_info("owner", &[]);
//     let msg = InstantiateMsg {  kernel_address: None };
//     let _res = instantiate(deps.as_mut(), env, &info, msg).unwrap();

//     let hook_msg = Cw721HookMsg::StartAuction {
//         start_time: Expiration::AtHeight(100),
//         end_time: Expiration::AtTime(Timestamp::from_seconds(200)),
//         coin_denom: "uusd".to_string(),
//         whitelist: None,
//         min_bid: None,
//     };
//     let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
//         sender: MOCK_TOKEN_OWNER.to_owned(),
//         token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
//         msg: encode_binary(&hook_msg).unwrap(),
//     });
//     let mut env = mock_env();
//     env.block.height = 0;

//     let info = mock_info(MOCK_TOKEN_ADDR, &[]);
//     let res = execute(deps.as_mut(), env, info, msg);

//     assert_eq!(
//         ContractError::ExpirationsMustBeOfSameType {},
//         res.unwrap_err()
//     );
// }

#[test]
fn execute_start_auction_start_time_in_past() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut());

    let hook_msg = Cw721HookMsg::StartAuction {
        start_time: Some(Expiry::AtTime(Milliseconds(100000))),
        end_time: Expiry::AtTime(Milliseconds(100000)),
        coin_denom: Asset::NativeToken("uusd".to_string()),
        whitelist: None,
        min_bid: None,
        min_raise: None,
        recipient: None,
        buy_now_price: None,
    };
    let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_owned(),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        msg: encode_binary(&hook_msg).unwrap(),
    });
    let env = mock_env();

    let info = mock_info(MOCK_TOKEN_ADDR, &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg);

    assert_eq!(
        ContractError::StartTimeInThePast {
            current_time: env.block.time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO,
            current_block: env.block.height,
        },
        res.unwrap_err()
    );
}

#[test]
fn execute_start_auction_zero_start_time() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut());

    let hook_msg = Cw721HookMsg::StartAuction {
        start_time: Some(Expiry::AtTime(Milliseconds::zero())),
        end_time: Expiry::AtTime(Milliseconds(1)),
        coin_denom: Asset::NativeToken("uusd".to_string()),
        whitelist: None,
        min_bid: None,
        min_raise: None,
        recipient: None,
        buy_now_price: None,
    };
    let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_owned(),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        msg: encode_binary(&hook_msg).unwrap(),
    });

    let info = mock_info(MOCK_TOKEN_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        ContractError::StartTimeInThePast {
            current_time: 1571797419879,
            current_block: 12345
        },
        res.unwrap_err()
    );
}

#[test]
fn execute_start_auction_start_time_not_provided() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut());

    let hook_msg = Cw721HookMsg::StartAuction {
        start_time: None,
        end_time: Expiry::AtTime(Milliseconds::from_nanos(
            (current_time() + 20_000_000) * 1_000_000,
        )),
        coin_denom: Asset::NativeToken("uusd".to_string()),
        whitelist: None,
        min_bid: None,
        min_raise: None,
        recipient: None,
        buy_now_price: None,
    };
    let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_owned(),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        msg: encode_binary(&hook_msg).unwrap(),
    });

    let info = mock_info(MOCK_TOKEN_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    assert!(res.is_ok())
}

#[test]
fn execute_start_auction_zero_duration() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut());

    let hook_msg = Cw721HookMsg::StartAuction {
        start_time: Some(Expiry::AtTime(Milliseconds(100))),
        end_time: Expiry::AtTime(Milliseconds::zero()),
        coin_denom: Asset::NativeToken("uusd".to_string()),
        whitelist: None,
        min_bid: None,
        min_raise: None,
        recipient: None,
        buy_now_price: None,
    };
    let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_owned(),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        msg: encode_binary(&hook_msg).unwrap(),
    });
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(0);

    let info = mock_info(MOCK_TOKEN_ADDR, &[]);
    let res = execute(deps.as_mut(), env, info, msg);

    assert_eq!(ContractError::InvalidExpiration {}, res.unwrap_err());
}

// #[test]
// fn execute_start_auction_end_time_never() {
//     let mut deps = mock_dependencies_custom(&[]);
//     let env = mock_env();
//     let info = mock_info("owner", &[]);
//     let msg = InstantiateMsg {  kernel_address: None };
//     let _res = instantiate(deps.as_mut(), env, &info, msg).unwrap();

//     let hook_msg = Cw721HookMsg::StartAuction {
//         end_time: Expiration::Never {},
//         start_time: Expiration::AtTime(Timestamp::from_seconds(200)),
//         coin_denom: "uusd".to_string(),
//         whitelist: None,
//         min_bid: None,
//     };
//     let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
//         sender: MOCK_TOKEN_OWNER.to_owned(),
//         token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
//         msg: encode_binary(&hook_msg).unwrap(),
//     });
//     let mut env = mock_env();
//     env.block.time = Timestamp::from_seconds(0);

//     let info = mock_info(MOCK_TOKEN_ADDR, &[]);
//     let res = execute(deps.as_mut(), env, info, msg);

//     assert_eq!(ContractError::ExpirationMustNotBeNever {}, res.unwrap_err());
// }

#[test]
fn execute_update_auction_zero_start() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);

    let msg = ExecuteMsg::UpdateAuction {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
        start_time: Some(Expiry::AtTime(Milliseconds::zero())),
        end_time: Expiry::AtTime(Milliseconds(1)),
        coin_denom: Asset::NativeToken("uusd".to_string()),
        whitelist: None,
        min_bid: None,
        min_raise: None,
        buy_now_price: None,
        recipient: None,
    };
    let mut env = mock_env();
    env.block.time = env.block.time.minus_days(1);

    let info = mock_info(MOCK_TOKEN_OWNER, &[]);
    let res = execute(deps.as_mut(), env, info, msg);

    assert_eq!(
        ContractError::StartTimeInThePast {
            current_time: 1571711019879,
            current_block: 12345
        },
        res.unwrap_err()
    );
}

#[test]
fn execute_update_auction_zero_duration() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);

    let msg = ExecuteMsg::UpdateAuction {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
        start_time: Some(Expiry::AtTime(Milliseconds(100000))),
        end_time: Expiry::AtTime(Milliseconds::zero()),
        coin_denom: Asset::NativeToken("uusd".to_string()),
        whitelist: None,
        min_bid: None,
        min_raise: None,
        buy_now_price: None,
        recipient: None,
    };
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(0);

    let info = mock_info(MOCK_TOKEN_OWNER, &[]);
    let res = execute(deps.as_mut(), env, info, msg);

    assert_eq!(ContractError::InvalidExpiration {}, res.unwrap_err());
}

#[test]
fn execute_update_auction_unauthorized() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);

    let msg = ExecuteMsg::UpdateAuction {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
        start_time: Some(Expiry::AtTime(Milliseconds(100000))),
        end_time: Expiry::AtTime(Milliseconds(100)),
        coin_denom: Asset::NativeToken("uusd".to_string()),
        whitelist: Some(vec![Addr::unchecked("user")]),
        min_bid: None,
        min_raise: None,
        buy_now_price: None,
        recipient: None,
    };
    let env = mock_env();

    let info = mock_info("not_owner", &[]);
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
}

#[test]
fn execute_update_auction_auction_started() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);

    let msg = ExecuteMsg::UpdateAuction {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
        start_time: Some(Expiry::AtTime(Milliseconds(100000))),
        end_time: Expiry::AtTime(Milliseconds(100)),
        coin_denom: Asset::NativeToken("uusd".to_string()),
        whitelist: Some(vec![Addr::unchecked("user")]),
        min_bid: None,
        min_raise: None,
        buy_now_price: None,
        recipient: None,
    };
    let mut env = mock_env();

    let info = mock_info(MOCK_TOKEN_OWNER, &[]);
    env.block.time = env.block.time.plus_days(1);

    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::AuctionAlreadyStarted {}, res.unwrap_err());
}

#[test]
fn execute_update_auction() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);

    let msg = ExecuteMsg::UpdateAuction {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
        start_time: Some(Expiry::AtTime(Milliseconds(1571711019879 + 1))),
        end_time: Expiry::AtTime(Milliseconds(1571711019879 + 2)),
        coin_denom: Asset::NativeToken("uusd".to_string()),
        whitelist: Some(vec![Addr::unchecked("user")]),
        min_bid: None,
        min_raise: None,
        buy_now_price: Some(Uint128::from(100u128)),
        recipient: None,
    };
    let mut env = mock_env();

    env.block.time = env.block.time.minus_days(1);

    let info = mock_info(MOCK_TOKEN_OWNER, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        TokenAuctionState {
            start_time: Expiration::AtTime(Timestamp::from_nanos(1571711019880000000)),
            end_time: Expiration::AtTime(Timestamp::from_nanos(1571711019881000000)),
            high_bidder_addr: Addr::unchecked(""),
            high_bidder_amount: Uint128::zero(),
            coin_denom: "uusd".to_string(),
            buy_now_price: Some(Uint128::from(100u128)),
            uses_cw20: false,
            auction_id: 1u128.into(),
            owner: MOCK_TOKEN_OWNER.to_string(),
            token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
            token_address: MOCK_TOKEN_ADDR.to_owned(),
            is_cancelled: false,
            is_bought: false,
            min_bid: None,
            min_raise: None,
            whitelist: Some(vec![Addr::unchecked("user")]),
            recipient: None,
        },
        TOKEN_AUCTION_STATE
            .load(deps.as_ref().storage, 1u128)
            .unwrap()
    );
}

#[test]
fn execute_start_auction_after_previous_finished() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut());

    // There was a previous auction.
    start_auction(deps.as_mut(), None, None, None, None);

    let hook_msg = Cw721HookMsg::StartAuction {
        start_time: None,
        end_time: Expiry::AtTime(Milliseconds::from_nanos(
            (current_time() + 20_000_000) * 1_000_000,
        )),
        coin_denom: Asset::NativeToken("uusd".to_string()),
        whitelist: None,
        min_bid: None,
        min_raise: None,
        recipient: None,
        buy_now_price: None,
    };
    let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_owned(),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        msg: encode_binary(&hook_msg).unwrap(),
    });
    let mut env = mock_env();
    // Auction ended by that time
    env.block.time = env.block.time.plus_hours(1);

    let info = mock_info(MOCK_TOKEN_ADDR, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        Response::new().add_attributes(vec![
            attr("action", "start_auction"),
            attr("start_time", "expiration time: 1571801019.880000000"),
            attr("end_time", "expiration time: 1571817419.879000000"),
            attr("coin_denom", "uusd"),
            attr("auction_id", "2"),
            attr("whitelist", "None"),
        ]),
        res
    );
}

#[test]
fn execute_claim_no_bids() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);

    // Auction ended by that time
    env.block.time = env.block.time.plus_days(1);

    let msg = ExecuteMsg::Claim {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("any_user", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_TOKEN_ADDR.to_owned(),
                msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                    recipient: AndrAddr::from_string(MOCK_TOKEN_OWNER.to_owned()),
                    token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
                })
                .unwrap(),
                funds: vec![],
            }))
            .add_attribute("action", "claim")
            .add_attribute("token_id", MOCK_UNCLAIMED_TOKEN)
            .add_attribute("token_contract", MOCK_TOKEN_ADDR)
            .add_attribute("recipient", MOCK_TOKEN_OWNER)
            .add_attribute("winning_bid_amount", Uint128::zero())
            .add_attribute("auction_id", "1"),
        res
    );
}

#[test]
fn execute_claim_no_bids_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init_cw20(deps.as_mut(), None);

    start_auction_cw20(deps.as_mut(), None, None, None, None);

    // Auction ended by that time
    env.block.time = env.block.time.plus_days(1);

    let msg = ExecuteMsg::Claim {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("any_user", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_TOKEN_ADDR.to_owned(),
                msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                    recipient: AndrAddr::from_string(MOCK_TOKEN_OWNER.to_owned()),
                    token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
                })
                .unwrap(),
                funds: vec![],
            }))
            .add_attribute("action", "claim")
            .add_attribute("token_id", MOCK_UNCLAIMED_TOKEN)
            .add_attribute("token_contract", MOCK_TOKEN_ADDR)
            .add_attribute("recipient", MOCK_TOKEN_OWNER)
            .add_attribute("winning_bid_amount", Uint128::zero())
            .add_attribute("auction_id", "1"),
        res
    );
}

#[test]
fn execute_claim_with_tax() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());
    let tax_recipient = "tax_recipient";

    let rate: Rate = Rate::Local(LocalRate {
        rate_type: LocalRateType::Additive,
        recipient: Recipient {
            address: AndrAddr::from_string(tax_recipient.to_string()),
            msg: None,
            ibc_recovery_address: None,
        },
        value: LocalRateValue::Flat(coin(20_u128, "uusd")),
        description: None,
    });

    // Set rates
    ADOContract::default()
        .set_rates(deps.as_mut().storage, "Claim", rate)
        .unwrap();

    start_auction(deps.as_mut(), None, None, None, None);

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    env.block.time = env.block.time.plus_seconds(1);

    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Auction ended by that time
    env.block.time = env.block.time.plus_days(1);

    let msg = ExecuteMsg::Claim {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("any_user", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    let transfer_nft_msg = Cw721ExecuteMsg::TransferNft {
        recipient: AndrAddr::from_string("sender".to_string()),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
    };
    assert_eq!(
        Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_TOKEN_ADDR.to_string(),
                msg: encode_binary(&transfer_nft_msg).unwrap(),
                funds: vec![],
            }))
            .add_message(CosmosMsg::Bank(BankMsg::Send {
                to_address: tax_recipient.to_owned(),
                amount: coins(20, "uusd"),
            }))
            .add_message(CosmosMsg::Bank(BankMsg::Send {
                to_address: MOCK_TOKEN_OWNER.to_owned(),
                amount: coins(100, "uusd"),
            }))
            .add_attribute("action", "claim")
            .add_attribute("token_id", MOCK_UNCLAIMED_TOKEN)
            .add_attribute("token_contract", MOCK_TOKEN_ADDR)
            .add_attribute("recipient", "sender")
            .add_attribute("winning_bid_amount", Uint128::from(100u128))
            .add_attribute("auction_id", "1"),
        res
    );
}

#[test]
fn execute_buy_now() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, Some(Uint128::new(500)));

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    env.block.time = env.block.time.plus_seconds(1);

    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let msg = ExecuteMsg::BuyNow {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("sender_2", &coins(500, "uusd".to_string()));
    env.block.time = env.block.time.plus_seconds(1);

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let transfer_nft_msg = Cw721ExecuteMsg::TransferNft {
        recipient: AndrAddr::from_string("sender_2".to_string()),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
    };
    assert_eq!(
        Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_TOKEN_ADDR.to_string(),
                msg: encode_binary(&transfer_nft_msg).unwrap(),
                funds: vec![],
            }))
            // .add_message(CosmosMsg::Bank(BankMsg::Send {
            //     to_address: tax_recipient.to_owned(),
            //     amount: coins(20, "uusd"),
            // }))
            // Refund highest bidder
            .add_message(CosmosMsg::Bank(BankMsg::Send {
                to_address: "sender".to_owned(),
                amount: coins(100, "uusd"),
            }))
            // Send 500 to seller
            .add_message(CosmosMsg::Bank(BankMsg::Send {
                to_address: MOCK_TOKEN_OWNER.to_owned(),
                amount: coins(500, "uusd"),
            }))
            .add_attribute("action", "buy_now")
            .add_attribute("token_id", MOCK_UNCLAIMED_TOKEN)
            .add_attribute("token_contract", MOCK_TOKEN_ADDR)
            .add_attribute("recipient", "sender_2")
            .add_attribute("bought_at", Uint128::from(500u128))
            .add_attribute("auction_id", "1"),
        res
    );

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    let err = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::AuctionBought {});

    // Verify that `is_bought` is set to `true` in the auction state
    let auction_state = TOKEN_AUCTION_STATE
        .load(deps.as_ref().storage, 1u128)
        .unwrap();
    assert!(auction_state.is_bought);
}

#[test]
fn execute_claim_with_royalty() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());
    let royalty_recipient = "royalty_recipient";

    let rate: Rate = Rate::Local(LocalRate {
        rate_type: LocalRateType::Deductive,
        recipient: Recipient {
            address: AndrAddr::from_string(royalty_recipient.to_string()),
            msg: None,
            ibc_recovery_address: None,
        },
        value: LocalRateValue::Flat(coin(20_u128, "uusd")),
        description: None,
    });

    // Set rates
    ADOContract::default()
        .set_rates(deps.as_mut().storage, "Claim", rate)
        .unwrap();

    start_auction(deps.as_mut(), None, None, None, None);

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    env.block.time = env.block.time.plus_seconds(1);

    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Auction ended by that time
    env.block.time = env.block.time.plus_days(1);

    let msg = ExecuteMsg::Claim {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("any_user", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    let transfer_nft_msg = Cw721ExecuteMsg::TransferNft {
        recipient: AndrAddr::from_string("sender".to_string()),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
    };
    assert_eq!(
        Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_TOKEN_ADDR.to_string(),
                msg: encode_binary(&transfer_nft_msg).unwrap(),
                funds: vec![],
            }))
            .add_message(CosmosMsg::Bank(BankMsg::Send {
                to_address: royalty_recipient.to_owned(),
                amount: coins(20, "uusd"),
            }))
            .add_message(CosmosMsg::Bank(BankMsg::Send {
                to_address: MOCK_TOKEN_OWNER.to_owned(),
                amount: coins(80, "uusd"),
            }))
            .add_attribute("action", "claim")
            .add_attribute("token_id", MOCK_UNCLAIMED_TOKEN)
            .add_attribute("token_contract", MOCK_TOKEN_ADDR)
            .add_attribute("recipient", "sender")
            .add_attribute("winning_bid_amount", Uint128::from(100u128))
            .add_attribute("auction_id", "1"),
        res
    );
}

#[test]
fn execute_claim_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init_cw20(deps.as_mut(), None);

    start_auction_cw20(deps.as_mut(), None, None, None, None);

    let hook_msg = Cw20HookMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "sender".to_string(),
        amount: Uint128::new(100),
        msg: encode_binary(&hook_msg).unwrap(),
    });

    let info = mock_info(MOCK_CW20_CONTRACT, &[]);
    env.block.time = env.block.time.plus_seconds(1);

    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Auction ended by that time
    env.block.time = env.block.time.plus_days(1);

    let msg = ExecuteMsg::Claim {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("any_user", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    let transfer_nft_msg = Cw721ExecuteMsg::TransferNft {
        recipient: AndrAddr::from_string("sender".to_string()),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
    };
    assert_eq!(
        Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_TOKEN_ADDR.to_string(),
                msg: encode_binary(&transfer_nft_msg).unwrap(),
                funds: vec![],
            }))
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CW20_CONTRACT.to_string(),
                msg: encode_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: MOCK_TOKEN_OWNER.to_owned(),
                    amount: Uint128::new(100)
                })
                .unwrap(),
                funds: vec![]
            }))
            .add_attribute("action", "claim")
            .add_attribute("token_id", MOCK_UNCLAIMED_TOKEN)
            .add_attribute("token_contract", MOCK_TOKEN_ADDR)
            .add_attribute("recipient", "sender")
            .add_attribute("winning_bid_amount", Uint128::from(100u128))
            .add_attribute("auction_id", "1"),
        res
    );
}

#[test]
fn execute_claim_cw20_with_tax() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init_cw20(deps.as_mut(), None);
    let tax_recipient = "tax_recipient";
    let rate: Rate = Rate::Local(LocalRate {
        rate_type: LocalRateType::Additive,
        recipient: Recipient {
            address: AndrAddr::from_string(tax_recipient.to_string()),
            msg: None,
            ibc_recovery_address: None,
        },
        value: LocalRateValue::Percent(PercentRate {
            percent: Decimal::percent(20),
        }),
        description: None,
    });

    // Set rates
    ADOContract::default()
        .set_rates(deps.as_mut().storage, "Claim", rate)
        .unwrap();

    start_auction_cw20(deps.as_mut(), None, None, None, None);

    let hook_msg = Cw20HookMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "sender".to_string(),
        amount: Uint128::new(100),
        msg: encode_binary(&hook_msg).unwrap(),
    });

    let info = mock_info(MOCK_CW20_CONTRACT, &[]);
    env.block.time = env.block.time.plus_seconds(1);

    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Auction ended by that time
    env.block.time = env.block.time.plus_days(1);

    let msg = ExecuteMsg::Claim {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("any_user", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    let transfer_nft_msg = Cw721ExecuteMsg::TransferNft {
        recipient: AndrAddr::from_string("sender".to_string()),
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
    };
    assert_eq!(
        Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_TOKEN_ADDR.to_string(),
                msg: encode_binary(&transfer_nft_msg).unwrap(),
                funds: vec![],
            }))
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CW20_CONTRACT.to_string(),
                msg: encode_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: tax_recipient.to_owned(),
                    amount: Uint128::new(20)
                })
                .unwrap(),
                funds: vec![]
            }))
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CW20_CONTRACT.to_string(),
                msg: encode_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: MOCK_TOKEN_OWNER.to_owned(),
                    amount: Uint128::new(100)
                })
                .unwrap(),
                funds: vec![]
            }))
            .add_attribute("action", "claim")
            .add_attribute("token_id", MOCK_UNCLAIMED_TOKEN)
            .add_attribute("token_contract", MOCK_TOKEN_ADDR)
            .add_attribute("recipient", "sender")
            .add_attribute("winning_bid_amount", Uint128::from(100u128))
            .add_attribute("auction_id", "1"),
        res
    );
}

#[test]
fn execute_claim_auction_not_ended() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("sender", &coins(100, "uusd".to_string()));
    env.block.time = env.block.time.plus_seconds(1);

    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let msg = ExecuteMsg::Claim {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("any_user", &[]);
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::AuctionNotEnded {}, res.unwrap_err());
}

#[test]
fn execute_claim_auction_already_claimed() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut());

    let hook_msg = Cw721HookMsg::StartAuction {
        start_time: None,
        end_time: Expiry::AtTime(Milliseconds::from_nanos(
            (current_time() + 20_000_000) * 1_000_000,
        )),
        coin_denom: Asset::NativeToken("uusd".to_string()),
        whitelist: None,
        min_bid: None,
        min_raise: None,
        recipient: None,
        buy_now_price: None,
    };
    let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: MOCK_TOKEN_OWNER.to_owned(),
        token_id: "claimed_token".to_string(),
        msg: encode_binary(&hook_msg).unwrap(),
    });
    let mut env = mock_env();

    let info = mock_info(MOCK_TOKEN_ADDR, &[]);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Auction is over.
    env.block.time = env.block.time.plus_days(1);

    let msg = ExecuteMsg::Claim {
        token_id: "claimed_token".to_string(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("any_user", &[]);
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::AuctionAlreadyClaimed {}, res.unwrap_err());
}

#[test]
fn execute_cancel_no_bids() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);

    let msg = ExecuteMsg::CancelAuction {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info(MOCK_TOKEN_OWNER, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_TOKEN_ADDR.to_owned(),
            msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                recipient: AndrAddr::from_string(MOCK_TOKEN_OWNER.to_owned()),
                token_id: MOCK_UNCLAIMED_TOKEN.to_owned()
            })
            .unwrap(),
            funds: vec![],
        })),
        res
    );

    assert!(
        TOKEN_AUCTION_STATE
            .load(deps.as_ref().storage, 1u128)
            .unwrap()
            .is_cancelled
    );
}

#[test]
fn execute_cancel_no_bids_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let _res = init_cw20(deps.as_mut(), None);

    start_auction_cw20(deps.as_mut(), None, None, None, None);

    let msg = ExecuteMsg::CancelAuction {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info(MOCK_TOKEN_OWNER, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_TOKEN_ADDR.to_owned(),
            msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                recipient: AndrAddr::from_string(MOCK_TOKEN_OWNER.to_owned()),
                token_id: MOCK_UNCLAIMED_TOKEN.to_owned()
            })
            .unwrap(),
            funds: vec![],
        })),
        res
    );

    assert!(
        TOKEN_AUCTION_STATE
            .load(deps.as_ref().storage, 1u128)
            .unwrap()
            .is_cancelled
    );
}

#[test]
fn execute_cancel_with_bids() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("bidder", &coins(100, "uusd"));
    env.block.time = env.block.time.plus_seconds(1);

    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let msg = ExecuteMsg::CancelAuction {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info(MOCK_TOKEN_OWNER, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_TOKEN_ADDR.to_owned(),
                msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                    recipient: AndrAddr::from_string(MOCK_TOKEN_OWNER.to_owned()),
                    token_id: MOCK_UNCLAIMED_TOKEN.to_owned()
                })
                .unwrap(),
                funds: vec![],
            }))
            .add_message(CosmosMsg::Bank(BankMsg::Send {
                to_address: "bidder".to_string(),
                amount: coins(100, "uusd")
            })),
        res
    );

    assert!(
        TOKEN_AUCTION_STATE
            .load(deps.as_ref().storage, 1u128)
            .unwrap()
            .is_cancelled
    );
}

#[test]
fn execute_cancel_with_bids_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init_cw20(deps.as_mut(), None);

    start_auction_cw20(deps.as_mut(), None, None, None, None);

    // let msg = ExecuteMsg::PlaceBid {
    //     token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
    //     token_address: MOCK_TOKEN_ADDR.to_string(),
    // };

    let hook_msg = Cw20HookMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "bidder".to_string(),
        amount: Uint128::new(100),
        msg: encode_binary(&hook_msg).unwrap(),
    });

    let info = mock_info(MOCK_CW20_CONTRACT, &[]);
    env.block.time = env.block.time.plus_seconds(1);

    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let msg = ExecuteMsg::CancelAuction {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info(MOCK_TOKEN_OWNER, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_TOKEN_ADDR.to_owned(),
                msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                    recipient: AndrAddr::from_string(MOCK_TOKEN_OWNER.to_owned()),
                    token_id: MOCK_UNCLAIMED_TOKEN.to_owned()
                })
                .unwrap(),
                funds: vec![],
            }))
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CW20_CONTRACT.to_string(),
                msg: encode_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "bidder".to_owned(),
                    amount: Uint128::new(100)
                })
                .unwrap(),
                funds: vec![]
            })),
        res
    );

    assert!(
        TOKEN_AUCTION_STATE
            .load(deps.as_ref().storage, 1u128)
            .unwrap()
            .is_cancelled
    );
}

#[test]
fn execute_cancel_not_token_owner() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);

    let msg = ExecuteMsg::CancelAuction {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("anyone", &[]);
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
}

#[test]
fn execute_cancel_auction_ended() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(deps.as_mut(), None, None, None, None);

    let msg = ExecuteMsg::CancelAuction {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    env.block.time = env.block.time.plus_days(1);

    let info = mock_info(MOCK_TOKEN_OWNER, &[]);
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::AuctionEnded {}, res.unwrap_err());
}

#[test]
fn execute_bid_below_min_price() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let _res = init(deps.as_mut());

    start_auction(
        deps.as_mut(),
        None,
        Some(Uint128::from(100u128)),
        None,
        None,
    );

    let msg = ExecuteMsg::PlaceBid {
        token_id: MOCK_UNCLAIMED_TOKEN.to_owned(),
        token_address: MOCK_TOKEN_ADDR.to_string(),
    };

    let info = mock_info("bidder", &coins(10, "uusd"));
    env.block.time = env.block.time.plus_seconds(1);
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap_err();

    assert_eq!(
        res,
        ContractError::InvalidFunds {
            msg: "Must provide at least 100 uusd to bid".to_string()
        }
    );

    let info = mock_info("bidder", &coins(100, "uusd"));
    //Will error if invalid
    execute(deps.as_mut(), env, info, msg).unwrap();
}
