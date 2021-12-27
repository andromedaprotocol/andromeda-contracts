use super::mock_querier::{
    mock_asset_config_response, mock_dependencies_custom, mock_mint_config_response,
    mock_next_position_idx_response, mock_poll_response, mock_polls_response,
    mock_pool_info_response, mock_position_response, mock_positions_response,
    mock_reward_info_response, mock_shares_response, mock_staker_response,
    mock_staking_config_response, mock_voter_response, mock_voters_response, MOCK_MIRROR_GOV_ADDR,
    MOCK_MIRROR_MINT_ADDR, MOCK_MIRROR_STAKING_ADDR,
};
use crate::contract::{execute, instantiate, query};
use andromeda_protocol::mirror_wrapped_cdp::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MirrorGovCw20HookMsg,
    MirrorGovExecuteMsg, MirrorGovQueryMsg, MirrorMintCw20HookMsg, MirrorMintExecuteMsg,
    MirrorMintQueryMsg, MirrorStakingCw20HookMsg, MirrorStakingExecuteMsg, MirrorStakingQueryMsg,
    QueryMsg,
};
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{
    from_binary, to_binary, CosmosMsg, Decimal, Deps, DepsMut, MessageInfo, Response, Uint128,
    WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use mirror_protocol::{
    gov::{ConfigResponse as GovConfigResponse, StateResponse as GovStateResponse, VoteOption},
    mint::ConfigResponse as MintConfigResponse,
    staking::ConfigResponse as StakingConfigResponse,
};
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use terraswap::asset::{Asset, AssetInfo};

const TEST_TOKEN: &str = "TEST_TOKEN";
const TEST_AMOUNT: u128 = 100u128;

fn assert_mint_execute_msg(deps: DepsMut, info: MessageInfo, mirror_msg: MirrorMintExecuteMsg) {
    let msg = ExecuteMsg::MirrorMintExecuteMsg(mirror_msg.clone());
    let res = execute(deps, mock_env(), info.clone(), msg.clone()).unwrap();

    let execute_msg = WasmMsg::Execute {
        contract_addr: MOCK_MIRROR_MINT_ADDR.to_string(),
        funds: info.funds,
        msg: to_binary(&mirror_msg).unwrap(),
    };
    assert_eq!(
        Response::new().add_messages(vec![CosmosMsg::Wasm(execute_msg)]),
        res
    );
}

fn assert_mint_execute_cw20_msg(
    deps: DepsMut,
    info: MessageInfo,
    mirror_msg: MirrorMintCw20HookMsg,
) {
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: info.sender.to_string(),
        amount: Uint128::from(TEST_AMOUNT),
        msg: to_binary(&Cw20HookMsg::MirrorMintCw20HookMsg(mirror_msg.clone())).unwrap(),
    });
    let res = execute(deps, mock_env(), mock_info(TEST_TOKEN, &[]), msg.clone()).unwrap();
    let send_msg = Cw20ExecuteMsg::Send {
        contract: MOCK_MIRROR_MINT_ADDR.to_string(),
        amount: Uint128::from(TEST_AMOUNT),
        msg: to_binary(&mirror_msg).unwrap(),
    };
    let execute_msg = WasmMsg::Execute {
        contract_addr: TEST_TOKEN.to_string(),
        funds: vec![],
        msg: to_binary(&send_msg).unwrap(),
    };
    assert_eq!(
        Response::new().add_messages(vec![CosmosMsg::Wasm(execute_msg)]),
        res
    );
}

fn assert_staking_execute_msg(
    deps: DepsMut,
    info: MessageInfo,
    mirror_msg: MirrorStakingExecuteMsg,
) {
    let msg = ExecuteMsg::MirrorStakingExecuteMsg(mirror_msg.clone());
    let res = execute(deps, mock_env(), info.clone(), msg.clone()).unwrap();

    let execute_msg = WasmMsg::Execute {
        contract_addr: MOCK_MIRROR_STAKING_ADDR.to_string(),
        funds: info.funds,
        msg: to_binary(&mirror_msg).unwrap(),
    };
    assert_eq!(
        Response::new().add_messages(vec![CosmosMsg::Wasm(execute_msg)]),
        res
    );
}

fn assert_staking_execute_cw20_msg(
    deps: DepsMut,
    info: MessageInfo,
    mirror_msg: MirrorStakingCw20HookMsg,
) {
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: info.sender.to_string(),
        amount: Uint128::from(TEST_AMOUNT),
        msg: to_binary(&Cw20HookMsg::MirrorStakingCw20HookMsg(mirror_msg.clone())).unwrap(),
    });
    let res = execute(deps, mock_env(), mock_info(TEST_TOKEN, &[]), msg.clone()).unwrap();
    let send_msg = Cw20ExecuteMsg::Send {
        contract: MOCK_MIRROR_STAKING_ADDR.to_string(),
        amount: Uint128::from(TEST_AMOUNT),
        msg: to_binary(&mirror_msg).unwrap(),
    };
    let execute_msg = WasmMsg::Execute {
        contract_addr: TEST_TOKEN.to_string(),
        funds: vec![],
        msg: to_binary(&send_msg).unwrap(),
    };
    assert_eq!(
        Response::new().add_messages(vec![CosmosMsg::Wasm(execute_msg)]),
        res
    );
}

fn assert_gov_execute_msg(deps: DepsMut, info: MessageInfo, mirror_msg: MirrorGovExecuteMsg) {
    let msg = ExecuteMsg::MirrorGovExecuteMsg(mirror_msg.clone());
    let res = execute(deps, mock_env(), info.clone(), msg.clone()).unwrap();

    let execute_msg = WasmMsg::Execute {
        contract_addr: MOCK_MIRROR_GOV_ADDR.to_string(),
        funds: info.funds,
        msg: to_binary(&mirror_msg).unwrap(),
    };
    assert_eq!(
        Response::new().add_messages(vec![CosmosMsg::Wasm(execute_msg)]),
        res
    );
}

fn assert_query_msg<T: DeserializeOwned + Debug + PartialEq>(
    deps: Deps,
    msg: QueryMsg,
    expected_res: T,
) {
    let actual_res: T = from_binary(&query(deps, mock_env(), msg.clone()).unwrap()).unwrap();
    assert_eq!(expected_res, actual_res);
}

fn assert_gov_execute_cw20_msg(deps: DepsMut, info: MessageInfo, mirror_msg: MirrorGovCw20HookMsg) {
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: info.sender.to_string(),
        amount: Uint128::from(TEST_AMOUNT),
        msg: to_binary(&Cw20HookMsg::MirrorGovCw20HookMsg(mirror_msg.clone())).unwrap(),
    });
    let res = execute(deps, mock_env(), mock_info(TEST_TOKEN, &[]), msg.clone()).unwrap();
    let send_msg = Cw20ExecuteMsg::Send {
        contract: MOCK_MIRROR_GOV_ADDR.to_string(),
        amount: Uint128::from(TEST_AMOUNT),
        msg: to_binary(&mirror_msg).unwrap(),
    };
    let execute_msg = WasmMsg::Execute {
        contract_addr: TEST_TOKEN.to_string(),
        funds: vec![],
        msg: to_binary(&send_msg).unwrap(),
    };
    assert_eq!(
        Response::new().add_messages(vec![CosmosMsg::Wasm(execute_msg)]),
        res
    );
}

fn assert_intantiate(deps: DepsMut, info: MessageInfo) {
    let msg = InstantiateMsg {
        mirror_mint_contract: MOCK_MIRROR_MINT_ADDR.to_string(),
        mirror_staking_contract: MOCK_MIRROR_STAKING_ADDR.to_string(),
        mirror_gov_contract: MOCK_MIRROR_GOV_ADDR.to_string(),
    };
    let res = instantiate(deps, mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("owner", info.sender),
        res
    );
}

#[test]
fn test_instantiate() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);

    assert_intantiate(deps.as_mut(), info);

    // Verify that we can query the mirror mint contract.
    let msg = QueryMsg::MirrorMintQueryMsg(MirrorMintQueryMsg::Config {});
    let res: MintConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();
    assert_eq!(mock_mint_config_response(), res);

    // Verify that we can query the mirror staking contract.
    let msg = QueryMsg::MirrorStakingQueryMsg(MirrorStakingQueryMsg::Config {});
    let res: StakingConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();
    assert_eq!(mock_staking_config_response(), res);

    // Verify that we can query the mirror gov contract.
    let msg = QueryMsg::MirrorGovQueryMsg(MirrorGovQueryMsg::Config {});
    // Can't check equality for this one as GovConfigResponse doesn't derive Debug for some reason.
    // But unwrapping is enough to check that it was returned.
    let _res: GovConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

    // Verify that we can query our contract's config.
    let msg = QueryMsg::Config {};
    let res: ConfigResponse = from_binary(&query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();
    assert_eq!(
        ConfigResponse {
            mirror_mint_contract: MOCK_MIRROR_MINT_ADDR.to_string(),
            mirror_staking_contract: MOCK_MIRROR_STAKING_ADDR.to_string(),
            mirror_gov_contract: MOCK_MIRROR_GOV_ADDR.to_string()
        },
        res
    );
}

#[test]
fn test_mirror_mint_open_position() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorMintExecuteMsg::OpenPosition {
        collateral: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(10_u128),
        },
        asset_info: AssetInfo::Token {
            contract_addr: "token_address".to_string(),
        },
        collateral_ratio: Decimal::one(),
        short_params: None,
    };
    assert_mint_execute_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_mint_deposit() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorMintExecuteMsg::Deposit {
        collateral: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(10_u128),
        },
        position_idx: Uint128::from(1_u128),
    };

    assert_mint_execute_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_mint_withdraw() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorMintExecuteMsg::Withdraw {
        position_idx: Uint128::from(1_u128),
        collateral: None,
    };

    assert_mint_execute_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_mint_mint() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorMintExecuteMsg::Mint {
        asset: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(10_u128),
        },
        position_idx: Uint128::from(1_u128),
        short_params: None,
    };

    assert_mint_execute_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_mint_open_position_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorMintCw20HookMsg::OpenPosition {
        asset_info: AssetInfo::Token {
            contract_addr: TEST_TOKEN.to_string(),
        },
        collateral_ratio: Decimal::one(),
        short_params: None,
    };

    assert_mint_execute_cw20_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_mint_deposit_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorMintCw20HookMsg::Deposit {
        position_idx: Uint128::from(1u128),
    };

    assert_mint_execute_cw20_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_mint_burn_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorMintCw20HookMsg::Burn {
        position_idx: Uint128::from(1u128),
    };

    assert_mint_execute_cw20_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_mint_auction_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorMintCw20HookMsg::Auction {
        position_idx: Uint128::from(1u128),
    };

    assert_mint_execute_cw20_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_staking_unbond() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorStakingExecuteMsg::Unbond {
        asset_token: "asset_token".to_string(),
        amount: Uint128::from(1_u128),
    };

    assert_staking_execute_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_staking_withdraw() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorStakingExecuteMsg::Withdraw { asset_token: None };

    assert_staking_execute_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_staking_autostake() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorStakingExecuteMsg::AutoStake {
        assets: [
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(10_u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(10_u128),
            },
        ],
        slippage_tolerance: None,
    };

    assert_staking_execute_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_staking_bond_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorStakingCw20HookMsg::Bond {
        asset_token: TEST_TOKEN.to_string(),
    };

    assert_staking_execute_cw20_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_gov_castvote() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorGovExecuteMsg::CastVote {
        poll_id: 1_u64,
        amount: Uint128::from(1_u128),
        vote: VoteOption::Yes,
    };

    assert_gov_execute_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_gov_withdraw_voting_tokens() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorGovExecuteMsg::WithdrawVotingTokens { amount: None };

    assert_gov_execute_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_gov_withdraw_voting_rewards() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorGovExecuteMsg::WithdrawVotingRewards { poll_id: None };

    assert_gov_execute_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_gov_stake_voting_rewards() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorGovExecuteMsg::StakeVotingRewards { poll_id: None };

    assert_gov_execute_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_gov_end_poll() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorGovExecuteMsg::EndPoll { poll_id: 1_u64 };

    assert_gov_execute_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_gov_execute_poll() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorGovExecuteMsg::ExecutePoll { poll_id: 1_u64 };

    assert_gov_execute_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_gov_snapshot_poll() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorGovExecuteMsg::SnapshotPoll { poll_id: 1_u64 };

    assert_gov_execute_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_gov_stake_voting_tokens_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorGovCw20HookMsg::StakeVotingTokens {};

    assert_gov_execute_cw20_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_gov_create_poll_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info.clone());

    let mirror_msg = MirrorGovCw20HookMsg::CreatePoll {
        title: "title".to_string(),
        description: "description".to_string(),
        link: None,
        execute_msg: None,
    };

    assert_gov_execute_cw20_msg(deps.as_mut(), info, mirror_msg);
}

#[test]
fn test_mirror_mint_queries() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info);

    let msg = MirrorMintQueryMsg::AssetConfig {
        asset_token: "token".to_string(),
    };
    assert_query_msg(
        deps.as_ref(),
        QueryMsg::MirrorMintQueryMsg(msg),
        mock_asset_config_response(),
    );

    let msg = MirrorMintQueryMsg::Position {
        position_idx: Uint128::from(1u128),
    };
    assert_query_msg(
        deps.as_ref(),
        QueryMsg::MirrorMintQueryMsg(msg),
        mock_position_response(),
    );

    let msg = MirrorMintQueryMsg::Positions {
        owner_addr: None,
        asset_token: None,
        start_after: None,
        limit: None,
        order_by: None,
    };
    assert_query_msg(
        deps.as_ref(),
        QueryMsg::MirrorMintQueryMsg(msg),
        mock_positions_response(),
    );

    let msg = MirrorMintQueryMsg::NextPositionIdx {};
    assert_query_msg(
        deps.as_ref(),
        QueryMsg::MirrorMintQueryMsg(msg),
        mock_next_position_idx_response(),
    );
}

#[test]
fn test_mirror_staking_queries() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info);

    let msg = MirrorStakingQueryMsg::PoolInfo {
        asset_token: "asset_token".to_string(),
    };
    assert_query_msg(
        deps.as_ref(),
        QueryMsg::MirrorStakingQueryMsg(msg),
        mock_pool_info_response(),
    );

    let msg = MirrorStakingQueryMsg::RewardInfo {
        asset_token: None,
        staker_addr: "staker_addr".to_string(),
    };
    assert_query_msg(
        deps.as_ref(),
        QueryMsg::MirrorStakingQueryMsg(msg),
        mock_reward_info_response(),
    );
}

#[test]
fn test_mirror_gov_queries() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    assert_intantiate(deps.as_mut(), info);

    let msg = MirrorGovQueryMsg::State {};
    // This response doesn't implement Debug so we can't use the helper function.
    let _res: GovStateResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::MirrorGovQueryMsg(msg)).unwrap())
            .unwrap();

    let msg = MirrorGovQueryMsg::Staker {
        address: "staker_addr".to_string(),
    };
    assert_query_msg(
        deps.as_ref(),
        QueryMsg::MirrorGovQueryMsg(msg),
        mock_staker_response(),
    );

    let msg = MirrorGovQueryMsg::Poll { poll_id: 1u64 };
    assert_query_msg(
        deps.as_ref(),
        QueryMsg::MirrorGovQueryMsg(msg),
        mock_poll_response(),
    );

    let msg = MirrorGovQueryMsg::Polls {
        filter: None,
        start_after: None,
        limit: None,
        order_by: None,
    };
    assert_query_msg(
        deps.as_ref(),
        QueryMsg::MirrorGovQueryMsg(msg),
        mock_polls_response(),
    );

    let msg = MirrorGovQueryMsg::Voter {
        poll_id: 1u64,
        address: "address".to_string(),
    };
    assert_query_msg(
        deps.as_ref(),
        QueryMsg::MirrorGovQueryMsg(msg),
        mock_voter_response(),
    );

    let msg = MirrorGovQueryMsg::Voters {
        poll_id: 1u64,
        start_after: None,
        limit: None,
        order_by: None,
    };
    assert_query_msg(
        deps.as_ref(),
        QueryMsg::MirrorGovQueryMsg(msg),
        mock_voters_response(),
    );

    let msg = MirrorGovQueryMsg::Shares {
        start_after: None,
        limit: None,
        order_by: None,
    };
    assert_query_msg(
        deps.as_ref(),
        QueryMsg::MirrorGovQueryMsg(msg),
        mock_shares_response(),
    );
}
