use andromeda_std::{
    amp::addresses::AndrAddr,
    common::{
        expiration::{Expiry, MILLISECONDS_TO_NANOSECONDS_RATIO},
        Milliseconds,
    },
    error::ContractError,
    testing::mock_querier::MOCK_KERNEL_CONTRACT,
};
use cosmwasm_std::{
    coin, coins, from_json,
    testing::{mock_dependencies, mock_env, mock_info, MOCK_CONTRACT_ADDR},
    to_json_binary, Addr, BankMsg, Decimal, Decimal256, DepsMut, Response, Uint128, Uint256,
    WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::{
    contract::{execute, instantiate, query},
    state::{
        Staker, StakerRewardInfo, CONFIG, MAX_REWARD_TOKENS, REWARD_TOKENS, STAKERS,
        STAKER_REWARD_INFOS, STATE,
    },
    testing::mock_querier::mock_dependencies_custom,
};
use andromeda_fungible_tokens::cw20_staking::{
    AllocationConfig, AllocationState, Config, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg,
    RewardToken, RewardTokenUnchecked, RewardType, StakerResponse, State,
};
use cw_asset::{AssetInfo, AssetInfoUnchecked};

const MOCK_STAKING_TOKEN: &str = "staking_token";
const MOCK_INCENTIVE_TOKEN: &str = "incentive_token";
const MOCK_ALLOCATED_TOKEN: &str = "allocated_token";

fn init(
    deps: DepsMut,
    additional_rewards: Option<Vec<RewardTokenUnchecked>>,
) -> Result<Response, ContractError> {
    let info = mock_info("owner", &[]);

    let msg = InstantiateMsg {
        staking_token: AndrAddr::from_string(MOCK_STAKING_TOKEN.to_owned()),
        additional_rewards,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    instantiate(deps, mock_env(), info, msg)
}

#[test]
fn test_instantiate() {
    let mut deps = mock_dependencies();
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    let res = init(
        deps.as_mut(),
        Some(vec![
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::native("uusd"),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp),
            },
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::cw20("incentive_token"),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp),
            },
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::cw20("allocated_token"),
                init_timestamp: Expiry::AtTime(current_timestamp),
                allocation_config: Some(AllocationConfig {
                    till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(1)),
                    cycle_rewards: Uint128::new(100),
                    cycle_duration: Milliseconds::from_seconds(1),
                    reward_increase: None,
                }),
            },
        ]),
    )
    .unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("type", "cw20-staking")
            .add_attribute("kernel_address", MOCK_KERNEL_CONTRACT)
            .add_attribute("owner", "owner"),
        res
    );

    assert_eq!(
        Config {
            staking_token: AndrAddr::from_string(MOCK_STAKING_TOKEN.to_owned()),
            number_of_reward_tokens: 3,
        },
        CONFIG.load(deps.as_ref().storage).unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::zero(),
            asset_info: AssetInfo::native("uusd"),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::zero(),
                init_timestamp: current_timestamp,
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "native:uusd")
            .unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::zero(),
            asset_info: AssetInfo::cw20(Addr::unchecked("incentive_token")),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::zero(),
                init_timestamp: current_timestamp,
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "cw20:incentive_token")
            .unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::zero(),
            asset_info: AssetInfo::cw20(Addr::unchecked("allocated_token")),
            reward_type: RewardType::Allocated {
                init_timestamp: current_timestamp,

                allocation_config: AllocationConfig {
                    till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(1)),
                    cycle_rewards: Uint128::new(100),
                    cycle_duration: Milliseconds::from_seconds(1),
                    reward_increase: None,
                },
                allocation_state: AllocationState {
                    current_cycle: 0,
                    current_cycle_rewards: Uint128::new(100),
                    last_distributed: current_timestamp,
                }
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "cw20:allocated_token")
            .unwrap()
    );

    assert_eq!(
        State {
            total_share: Uint128::zero(),
        },
        STATE.load(deps.as_ref().storage).unwrap()
    );
}

#[test]
fn test_instantiate_exceed_max() {
    let mut deps = mock_dependencies();
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());

    let mut reward_tokens: Vec<RewardTokenUnchecked> = vec![];

    for i in 0..MAX_REWARD_TOKENS + 1 {
        reward_tokens.push(RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(format!("token{i}")),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        });
    }

    let res = init(deps.as_mut(), Some(reward_tokens));

    assert_eq!(
        ContractError::MaxRewardTokensExceeded {
            max: MAX_REWARD_TOKENS
        },
        res.unwrap_err()
    );
}

#[test]
fn test_instantiate_staking_token_as_addtional_reward() {
    let mut deps = mock_dependencies();
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());

    let res = init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_STAKING_TOKEN),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        }]),
    );
    assert_eq!(
        ContractError::InvalidAsset {
            asset: "staking_token".to_string()
        },
        res.unwrap_err()
    );
}

#[test]
fn test_instantiate_start_time_in_past() {
    let mut deps = mock_dependencies();
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());

    let res = init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
            init_timestamp: Expiry::AtTime(current_timestamp.minus_seconds(1)),
            allocation_config: Some(AllocationConfig {
                till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(1)),
                cycle_rewards: Uint128::new(100),
                cycle_duration: Milliseconds::from_seconds(1),
                reward_increase: None,
            }),
        }]),
    );

    let env = mock_env();
    assert_eq!(
        ContractError::StartTimeInThePast {
            current_block: env.block.height,
            current_time: env.block.time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO
        },
        res.unwrap_err()
    );
}

#[test]
fn test_instantiate_end_time_in_past() {
    let mut deps = mock_dependencies();
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());

    let res = init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
            init_timestamp: Expiry::AtTime(current_timestamp),

            allocation_config: Some(AllocationConfig {
                till_timestamp: Expiry::AtTime(current_timestamp.minus_seconds(1)),
                cycle_rewards: Uint128::new(100),
                cycle_duration: Milliseconds::from_seconds(1),
                reward_increase: None,
            }),
        }]),
    );

    assert_eq!(ContractError::StartTimeAfterEndTime {}, res.unwrap_err());
}

#[test]
fn test_instantiate_cycle_duration_zero() {
    let mut deps = mock_dependencies();
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());

    let res = init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
            init_timestamp: Expiry::AtTime(current_timestamp),

            allocation_config: Some(AllocationConfig {
                till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(1)),
                cycle_rewards: Uint128::new(100),
                cycle_duration: Milliseconds::zero(),
                reward_increase: None,
            }),
        }]),
    );

    assert_eq!(ContractError::InvalidCycleDuration {}, res.unwrap_err());
}

#[test]
fn test_instantiate_invalid_reward_increase() {
    let mut deps = mock_dependencies();
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());

    let res = init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
            init_timestamp: Expiry::AtTime(current_timestamp),

            allocation_config: Some(AllocationConfig {
                till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(1)),
                cycle_rewards: Uint128::new(100),
                cycle_duration: Milliseconds::from_seconds(1),
                reward_increase: Some(Decimal::one()),
            }),
        }]),
    );

    assert_eq!(ContractError::InvalidRewardIncrease {}, res.unwrap_err());
}

#[test]
fn test_receive_cw20_zero_amount() {
    let mut deps = mock_dependencies_custom(&[]);
    init(deps.as_mut(), None).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "sender".to_string(),
        amount: Uint128::zero(),
        msg: to_json_binary(&"").unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        ContractError::InvalidFunds {
            msg: "Amount must be non-zero".to_string()
        },
        res.unwrap_err()
    );
}

#[test]
fn test_stake_unstake_tokens() {
    let mut deps = mock_dependencies_custom(&[]);
    init(deps.as_mut(), None).unwrap();

    deps.querier.with_token_balances(&[(
        &MOCK_STAKING_TOKEN.to_string(),
        // 100 initial, 100 added by deposit.
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100 + 100))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "sender".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "stake_tokens")
            .add_attribute("sender", "sender")
            .add_attribute("share", "100")
            .add_attribute("amount", "100"),
        res
    );

    assert_eq!(
        State {
            total_share: Uint128::new(100)
        },
        STATE.load(deps.as_ref().storage).unwrap()
    );

    assert_eq!(
        Staker {
            share: Uint128::new(100)
        },
        STAKERS.load(deps.as_ref().storage, "sender").unwrap()
    );

    deps.querier.with_token_balances(&[(
        &MOCK_STAKING_TOKEN.to_string(),
        // 200 from last time and 100 for the deposit made by other_sender
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(200 + 100))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "other_sender".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "stake_tokens")
            .add_attribute("sender", "other_sender")
            .add_attribute("share", "50")
            .add_attribute("amount", "100"),
        res
    );

    assert_eq!(
        State {
            total_share: Uint128::new(150)
        },
        STATE.load(deps.as_ref().storage).unwrap()
    );

    assert_eq!(
        Staker {
            share: Uint128::new(50)
        },
        STAKERS.load(deps.as_ref().storage, "other_sender").unwrap()
    );

    // User 1 tries to unstake too many tokens.
    let msg = ExecuteMsg::UnstakeTokens {
        amount: Some(Uint128::new(202)),
    };

    let info = mock_info("sender", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg);

    assert_eq!(
        ContractError::InvalidWithdrawal {
            msg: Some("Desired amount exceeds balance".to_string()),
        },
        res.unwrap_err()
    );

    // User 1 unstakes all
    let msg = ExecuteMsg::UnstakeTokens { amount: None };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "unstake_tokens")
            .add_attribute("sender", "sender")
            .add_attribute("withdraw_amount", "200")
            .add_attribute("withdraw_share", "100")
            .add_message(WasmMsg::Execute {
                contract_addr: MOCK_STAKING_TOKEN.to_owned(),
                funds: vec![],
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "sender".to_string(),
                    amount: Uint128::new(200)
                })
                .unwrap()
            }),
        res
    );

    assert_eq!(
        State {
            total_share: Uint128::new(50)
        },
        STATE.load(deps.as_ref().storage).unwrap()
    );

    assert_eq!(
        Staker {
            share: Uint128::zero()
        },
        STAKERS.load(deps.as_ref().storage, "sender").unwrap()
    );

    deps.querier.with_token_balances(&[(
        &MOCK_STAKING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
    )]);

    // User 2 unstakes all
    let msg = ExecuteMsg::UnstakeTokens { amount: None };

    let info = mock_info("other_sender", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "unstake_tokens")
            .add_attribute("sender", "other_sender")
            .add_attribute("withdraw_amount", "100")
            .add_attribute("withdraw_share", "50")
            .add_message(WasmMsg::Execute {
                contract_addr: MOCK_STAKING_TOKEN.to_owned(),
                funds: vec![],
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "other_sender".to_string(),
                    amount: Uint128::new(100)
                })
                .unwrap()
            }),
        res
    );

    assert_eq!(
        State {
            total_share: Uint128::zero()
        },
        STATE.load(deps.as_ref().storage).unwrap()
    );

    assert_eq!(
        Staker {
            share: Uint128::zero()
        },
        STAKERS.load(deps.as_ref().storage, "other_sender").unwrap()
    );
}

#[test]
fn test_stake_invalid_token() {
    let mut deps = mock_dependencies_custom(&[]);
    init(deps.as_mut(), None).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "sender".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info("invalid_token", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        ContractError::InvalidFunds {
            msg: "Deposited cw20 token is not the staking token".to_string(),
        },
        res.unwrap_err()
    );
}

#[test]
fn test_update_global_indexes() {
    let mut deps = mock_dependencies_custom(&[coin(40, "uusd"), coin(40, "uandr")]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::native("uusd"),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp),
            },
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::native("uandr"),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(1)),
            },
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp),
            },
        ]),
    )
    .unwrap();

    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
        (
            &MOCK_INCENTIVE_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(20))],
        ),
    ]);

    STATE
        .save(
            deps.as_mut().storage,
            &State {
                total_share: Uint128::new(100),
            },
        )
        .unwrap();

    let msg = ExecuteMsg::UpdateGlobalIndexes { asset_infos: None };

    let info = mock_info("owner", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "update_global_indexes")
            .add_attribute("cw20:incentive_token", "0.2")
            .add_attribute("native:uandr", "0")
            .add_attribute("native:uusd", "0.4"),
        res
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(40u128), Uint256::from(100u128)),
            asset_info: AssetInfo::native("uusd"),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::new(40),
                init_timestamp: current_timestamp,
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "native:uusd")
            .unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::zero(),
            asset_info: AssetInfo::native("uandr"),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::zero(),
                init_timestamp: current_timestamp.plus_seconds(1),
            },
            is_active: true
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "native:uandr")
            .unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(20u128), Uint256::from(100u128)),
            asset_info: AssetInfo::cw20(Addr::unchecked(MOCK_INCENTIVE_TOKEN)),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::new(20),
                init_timestamp: current_timestamp,
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "cw20:incentive_token")
            .unwrap()
    );

    // Check unallocate updates after init timestamp
    let msg = ExecuteMsg::UpdateGlobalIndexes { asset_infos: None };
    let mut new_env = mock_env();
    new_env.block.time = new_env.block.time.plus_seconds(2);
    let res = execute(deps.as_mut(), new_env, info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "update_global_indexes")
            .add_attribute("cw20:incentive_token", "0.2")
            .add_attribute("native:uandr", "0.4")
            .add_attribute("native:uusd", "0.4"),
        res
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(40u128), Uint256::from(100u128)),
            asset_info: AssetInfo::native("uandr"),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::new(40),
                init_timestamp: current_timestamp.plus_seconds(1),
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "native:uandr")
            .unwrap()
    );
}

#[test]
fn test_update_global_indexes_selective() {
    let mut deps = mock_dependencies_custom(&coins(40, "uusd"));
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::native("uusd"),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp),
            },
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp),
            },
        ]),
    )
    .unwrap();

    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
        (
            &MOCK_INCENTIVE_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(20))],
        ),
    ]);

    STATE
        .save(
            deps.as_mut().storage,
            &State {
                total_share: Uint128::new(100),
            },
        )
        .unwrap();

    let msg = ExecuteMsg::UpdateGlobalIndexes {
        asset_infos: Some(vec![AssetInfoUnchecked::native("uusd")]),
    };

    let info = mock_info("owner", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "update_global_indexes")
            .add_attribute("native:uusd", "0.4"),
        res
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(40u128), Uint256::from(100u128)),
            asset_info: AssetInfo::native("uusd"),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::new(40),
                init_timestamp: current_timestamp,
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "native:uusd")
            .unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::zero(),
            asset_info: AssetInfo::cw20(Addr::unchecked(MOCK_INCENTIVE_TOKEN)),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::zero(),
                init_timestamp: current_timestamp,
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "cw20:incentive_token")
            .unwrap()
    );
}

#[test]
fn test_update_global_indexes_invalid_asset() {
    let mut deps = mock_dependencies_custom(&coins(40, "uusd"));
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::native("uusd"),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp),
            },
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp),
            },
        ]),
    )
    .unwrap();

    STATE
        .save(
            deps.as_mut().storage,
            &State {
                total_share: Uint128::new(100),
            },
        )
        .unwrap();

    let msg = ExecuteMsg::UpdateGlobalIndexes {
        asset_infos: Some(vec![AssetInfoUnchecked::native("uluna")]),
    };

    let info = mock_info("owner", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        ContractError::InvalidAsset {
            asset: "native:uluna".to_string(),
        },
        res.unwrap_err()
    );
}

#[test]
fn test_update_global_indexes_cw20_deposit() {
    let mut deps = mock_dependencies_custom(&coins(40, "uusd"));
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::native("uusd"),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp),
            },
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp),
            },
        ]),
    )
    .unwrap();

    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
        (
            &MOCK_INCENTIVE_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(20))],
        ),
    ]);

    STATE
        .save(
            deps.as_mut().storage,
            &State {
                total_share: Uint128::new(100),
            },
        )
        .unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "owner".to_string(),
        amount: Uint128::new(20),
        msg: to_json_binary(&Cw20HookMsg::UpdateGlobalIndex {}).unwrap(),
    });

    let info = mock_info(MOCK_INCENTIVE_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "update_global_indexes")
            .add_attribute("cw20:incentive_token", "0.2"),
        res
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::zero(),
            asset_info: AssetInfo::native("uusd"),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::zero(),
                init_timestamp: current_timestamp,
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "native:uusd")
            .unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(20u128), Uint256::from(100u128)),
            asset_info: AssetInfo::cw20(Addr::unchecked(MOCK_INCENTIVE_TOKEN)),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::new(20),
                init_timestamp: current_timestamp,
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "cw20:incentive_token")
            .unwrap()
    );
}

#[test]
fn test_claim_rewards() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::native("uusd"),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        }]),
    )
    .unwrap();

    deps.querier.with_token_balances(&[(
        &MOCK_STAKING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100 + 100))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user1".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    deps.querier.with_token_balances(&[(
        &MOCK_STAKING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(200 + 100))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user2".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Staker {
            share: Uint128::new(100)
        },
        STAKERS.load(deps.as_ref().storage, "user1").unwrap()
    );
    assert_eq!(
        Staker {
            share: Uint128::new(50)
        },
        STAKERS.load(deps.as_ref().storage, "user2").unwrap()
    );

    let info = mock_info("user1", &[]);
    let msg = ExecuteMsg::ClaimRewards {};
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    // No rewards have been given yet.
    assert_eq!(ContractError::WithdrawalIsEmpty {}, res.unwrap_err());

    deps.querier
        .base
        .update_balance(mock_env().contract.address, coins(100, "uusd"));

    // Update the global index for uusd by depositing 100 uusd
    let msg = ExecuteMsg::UpdateGlobalIndexes {
        asset_infos: Some(vec![AssetInfoUnchecked::native("uusd")]),
    };

    let info = mock_info("owner", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(100u128), Uint256::from(150u128)),
            asset_info: AssetInfo::native("uusd"),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::new(100),
                init_timestamp: current_timestamp,
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "native:uusd")
            .unwrap()
    );

    // Verify that the queries return the updated rewards.
    let msg = QueryMsg::Stakers {
        start_after: None,
        limit: None,
    };
    let res: Vec<StakerResponse> =
        from_json(query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

    assert_eq!(
        vec![
            StakerResponse {
                address: "user1".to_string(),
                share: Uint128::new(100),
                pending_rewards: vec![("native:uusd".to_string(), Uint128::new(66))],
                balance: Uint128::new(200),
            },
            StakerResponse {
                address: "user2".to_string(),
                share: Uint128::new(50),
                pending_rewards: vec![("native:uusd".to_string(), Uint128::new(33))],
                balance: Uint128::new(100),
            },
        ],
        res
    );

    let info = mock_info("user1", &[]);
    let msg = ExecuteMsg::ClaimRewards {};
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::from_ratio(Uint256::from(100u128), Uint256::from(150u128)),
            pending_rewards: Decimal256::zero(),
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user1", "native:uusd"))
            .unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(100u128), Uint256::from(150u128)),
            asset_info: AssetInfo::native("uusd"),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::new(34),
                init_timestamp: current_timestamp,
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "native:uusd")
            .unwrap()
    );

    assert_eq!(
        Response::new()
            .add_attribute("action", "claim_rewards")
            .add_message(BankMsg::Send {
                to_address: "user1".to_string(),
                amount: coins(66, "uusd")
            }),
        res.unwrap()
    );

    deps.querier
        .base
        .update_balance(mock_env().contract.address, coins(34, "uusd"));

    let info = mock_info("user2", &[]);
    let msg = ExecuteMsg::ClaimRewards {};
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        Response::new()
            .add_attribute("action", "claim_rewards")
            .add_message(BankMsg::Send {
                to_address: "user2".to_string(),
                amount: coins(33, "uusd")
            }),
        res.unwrap()
    );

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::from_ratio(Uint256::from(100u128), Uint256::from(150u128)),
            pending_rewards: Decimal256::zero(),
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user2", "native:uusd"))
            .unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(100u128), Uint256::from(150u128)),
            asset_info: AssetInfo::native("uusd"),
            reward_type: RewardType::NonAllocated {
                // Small rounding error, shouldn't really make a difference and is inevitable.
                previous_reward_balance: Uint128::new(1),
                init_timestamp: current_timestamp,
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "native:uusd")
            .unwrap()
    );

    deps.querier
        .base
        .update_balance(mock_env().contract.address, coins(1, "uusd"));

    // Verify that the queries return the correct pending rewards.
    let msg = QueryMsg::Stakers {
        start_after: None,
        limit: None,
    };
    let res: Vec<StakerResponse> =
        from_json(query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

    assert_eq!(
        vec![
            StakerResponse {
                address: "user1".to_string(),
                share: Uint128::new(100),
                pending_rewards: vec![("native:uusd".to_string(), Uint128::zero())],
                balance: Uint128::new(200),
            },
            StakerResponse {
                address: "user2".to_string(),
                share: Uint128::new(50),
                pending_rewards: vec![("native:uusd".to_string(), Uint128::zero())],
                balance: Uint128::new(100),
            },
        ],
        res
    );
}

#[test]
fn test_claim_rewards_allocated() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_ALLOCATED_TOKEN),
            init_timestamp: Expiry::AtTime(current_timestamp),

            allocation_config: Some(AllocationConfig {
                till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(100)),
                cycle_rewards: Uint128::new(100),
                cycle_duration: Milliseconds::from_seconds(100),
                reward_increase: None,
            }),
        }]),
    )
    .unwrap();

    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            // 100 is user's deposit.
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
        (
            &MOCK_ALLOCATED_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
    ]);

    // User 1 stakes tokens.
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user1".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::zero(),
            pending_rewards: Decimal256::zero(),
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user1", "cw20:allocated_token"))
            .unwrap()
    );

    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100 + 100))],
        ),
        (
            &MOCK_ALLOCATED_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
    ]);

    // User 2 stakes 100 tokens.
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user2".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Staker {
            share: Uint128::new(100)
        },
        STAKERS.load(deps.as_ref().storage, "user1").unwrap()
    );
    assert_eq!(
        Staker {
            share: Uint128::new(100)
        },
        STAKERS.load(deps.as_ref().storage, "user2").unwrap()
    );

    // Speed time up to halfway through cycle.
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(50);

    // User 1 claims rewards.
    let info = mock_info("user1", &[]);
    let msg = ExecuteMsg::ClaimRewards {};
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "claim_rewards")
            .add_message(WasmMsg::Execute {
                contract_addr: MOCK_ALLOCATED_TOKEN.to_owned(),
                funds: vec![],
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "user1".to_string(),
                    amount: Uint128::new(25)
                })
                .unwrap(),
            }),
        res
    );

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::percent(25),
            pending_rewards: Decimal256::zero(),
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user1", "cw20:allocated_token"))
            .unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(50u128), Uint256::from(200u128)),
            asset_info: AssetInfo::cw20(Addr::unchecked(MOCK_ALLOCATED_TOKEN)),
            reward_type: RewardType::Allocated {
                init_timestamp: current_timestamp,

                allocation_config: AllocationConfig {
                    till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(100)),
                    cycle_rewards: Uint128::new(100),
                    cycle_duration: Milliseconds::from_seconds(100),
                    reward_increase: None,
                },
                allocation_state: AllocationState {
                    current_cycle: 0,
                    current_cycle_rewards: Uint128::new(100),
                    last_distributed: current_timestamp.plus_seconds(50),
                },
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "cw20:allocated_token")
            .unwrap()
    );

    // User 2 claims rewards.
    let info = mock_info("user2", &[]);
    let msg = ExecuteMsg::ClaimRewards {};
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "claim_rewards")
            .add_message(WasmMsg::Execute {
                contract_addr: MOCK_ALLOCATED_TOKEN.to_owned(),
                funds: vec![],
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "user2".to_string(),
                    amount: Uint128::new(25)
                })
                .unwrap(),
            }),
        res
    );

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::percent(25),
            pending_rewards: Decimal256::zero(),
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user2", "cw20:allocated_token"))
            .unwrap()
    );
}

#[test]
fn test_claim_rewards_allocated_init_timestamp_in_future() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_ALLOCATED_TOKEN),
            init_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(10)),
            allocation_config: Some(AllocationConfig {
                till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(110)),
                cycle_rewards: Uint128::new(100),
                cycle_duration: Milliseconds::from_seconds(100),
                reward_increase: None,
            }),
        }]),
    )
    .unwrap();

    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            // 100 is user's deposit.
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
        (
            &MOCK_ALLOCATED_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
    ]);

    // User 1 stakes tokens.
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user1".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::zero(),
            pending_rewards: Decimal256::zero(),
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user1", "cw20:allocated_token"))
            .unwrap()
    );

    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100 + 100))],
        ),
        (
            &MOCK_ALLOCATED_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
    ]);

    // User 2 stakes 100 tokens.
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user2".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Staker {
            share: Uint128::new(100)
        },
        STAKERS.load(deps.as_ref().storage, "user1").unwrap()
    );
    assert_eq!(
        Staker {
            share: Uint128::new(100)
        },
        STAKERS.load(deps.as_ref().storage, "user2").unwrap()
    );

    // Speed time up to halfway through cycle.

    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(50 + 10);

    // User 1 claims rewards.
    let info = mock_info("user1", &[]);
    let msg = ExecuteMsg::ClaimRewards {};
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "claim_rewards")
            .add_message(WasmMsg::Execute {
                contract_addr: MOCK_ALLOCATED_TOKEN.to_owned(),
                funds: vec![],
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "user1".to_string(),
                    amount: Uint128::new(25)
                })
                .unwrap(),
            }),
        res
    );

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::percent(25),
            pending_rewards: Decimal256::zero(),
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user1", "cw20:allocated_token"))
            .unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(50u128), Uint256::from(200u128)),
            asset_info: AssetInfo::cw20(Addr::unchecked(MOCK_ALLOCATED_TOKEN)),
            reward_type: RewardType::Allocated {
                init_timestamp: current_timestamp.plus_seconds(10),
                allocation_config: AllocationConfig {
                    till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(110)),
                    cycle_rewards: Uint128::new(100),
                    cycle_duration: Milliseconds::from_seconds(100),
                    reward_increase: None,
                },
                allocation_state: AllocationState {
                    current_cycle: 0,
                    current_cycle_rewards: Uint128::new(100),
                    last_distributed: current_timestamp.plus_seconds(60),
                },
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "cw20:allocated_token")
            .unwrap()
    );

    // User 2 claims rewards.
    let info = mock_info("user2", &[]);
    let msg = ExecuteMsg::ClaimRewards {};
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "claim_rewards")
            .add_message(WasmMsg::Execute {
                contract_addr: MOCK_ALLOCATED_TOKEN.to_owned(),
                funds: vec![],
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "user2".to_string(),
                    amount: Uint128::new(25)
                })
                .unwrap(),
            }),
        res
    );

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::percent(25),
            pending_rewards: Decimal256::zero(),
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user2", "cw20:allocated_token"))
            .unwrap()
    );
}

#[test]
fn test_stake_rewards_update() {
    let mut deps = mock_dependencies_custom(&coins(40, "uusd"));
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp),
            },
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::native("uusd"),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp),
            },
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::cw20(MOCK_ALLOCATED_TOKEN),
                init_timestamp: Expiry::AtTime(current_timestamp),

                allocation_config: Some(AllocationConfig {
                    till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(100)),
                    cycle_rewards: Uint128::new(100),
                    cycle_duration: Milliseconds::from_seconds(100),
                    reward_increase: None,
                }),
            },
        ]),
    )
    .unwrap();

    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
        (
            // Add allocated token.
            &MOCK_ALLOCATED_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
    ]);

    // Stake tokens.
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user1".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
        (
            // Deposit incentive token
            &MOCK_INCENTIVE_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(20))],
        ),
        (
            // Add allocated token.
            &MOCK_ALLOCATED_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
    ]);

    // Update global index.
    let msg = ExecuteMsg::UpdateGlobalIndexes { asset_infos: None };
    let info = mock_info("owner", &[]);

    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Verify pending rewards updated with query.
    let msg = QueryMsg::Staker {
        address: "user1".to_string(),
    };
    // Speed time up to halfway through cycle.
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(50);

    let res: StakerResponse = from_json(query(deps.as_ref(), env.clone(), msg).unwrap()).unwrap();

    assert_eq!(
        StakerResponse {
            address: "user1".to_string(),
            share: Uint128::new(100),
            pending_rewards: vec![
                ("cw20:allocated_token".to_string(), Uint128::new(50)),
                ("cw20:incentive_token".to_string(), Uint128::new(20)),
                ("native:uusd".to_string(), Uint128::new(40))
            ],
            balance: Uint128::new(100),
        },
        res
    );

    // Stake 50 more.
    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100 + 50))],
        ),
        (
            // Deposit incentive token
            &MOCK_INCENTIVE_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(20))],
        ),
        (
            // Add allocated token.
            &MOCK_ALLOCATED_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user1".to_string(),
        amount: Uint128::new(50),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::from_ratio(Uint256::from(20u128), Uint256::from(100u128)),
            pending_rewards: Decimal256::from_ratio(Uint256::from(20u128), 1u128)
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user1", "cw20:incentive_token"))
            .unwrap()
    );

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::from_ratio(Uint256::from(40u128), Uint256::from(100u128)),
            pending_rewards: Decimal256::from_ratio(Uint256::from(40u128), 1u128)
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user1", "native:uusd"))
            .unwrap()
    );

    assert_eq!(
        StakerRewardInfo {
            // Halfway through cycle -> half of rewards available
            index: Decimal256::from_ratio(Uint256::from(50u128), Uint256::from(100u128)),
            pending_rewards: Decimal256::from_ratio(Uint256::from(50u128), 1u128)
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user1", "cw20:allocated_token"))
            .unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(50u128), Uint256::from(100u128)),
            asset_info: AssetInfo::cw20(Addr::unchecked(MOCK_ALLOCATED_TOKEN)),
            reward_type: RewardType::Allocated {
                init_timestamp: current_timestamp,

                allocation_config: AllocationConfig {
                    till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(100)),
                    cycle_rewards: Uint128::new(100),
                    cycle_duration: Milliseconds::from_seconds(100),
                    reward_increase: None,
                },
                allocation_state: AllocationState {
                    current_cycle: 0,
                    current_cycle_rewards: Uint128::new(100),
                    last_distributed: current_timestamp.plus_seconds(50),
                },
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "cw20:allocated_token")
            .unwrap()
    );
}

#[test]
fn test_unstake_rewards_update() {
    let mut deps = mock_dependencies_custom(&coins(40, "uusd"));
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp),
            },
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::native("uusd"),
                allocation_config: None,
                init_timestamp: Expiry::AtTime(current_timestamp),
            },
            RewardTokenUnchecked {
                asset_info: AssetInfoUnchecked::cw20(MOCK_ALLOCATED_TOKEN),
                init_timestamp: Expiry::AtTime(current_timestamp),

                allocation_config: Some(AllocationConfig {
                    till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(100)),
                    cycle_rewards: Uint128::new(100),
                    cycle_duration: Milliseconds::from_seconds(100),
                    reward_increase: None,
                }),
            },
        ]),
    )
    .unwrap();

    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
        (
            // Add allocated token.
            &MOCK_ALLOCATED_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user1".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
        (
            // Deposit incentive token
            &MOCK_INCENTIVE_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(20))],
        ),
        (
            &MOCK_ALLOCATED_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
    ]);

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::zero(),
            pending_rewards: Decimal256::zero()
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user1", "cw20:allocated_token"))
            .unwrap()
    );

    // Update global index.
    let msg = ExecuteMsg::UpdateGlobalIndexes { asset_infos: None };
    let info = mock_info("owner", &[]);

    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Unstake all.
    let msg = ExecuteMsg::UnstakeTokens { amount: None };

    let info = mock_info("user1", &[]);

    // Speed time up to halfway through cycle.
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(50);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::from_ratio(Uint256::from(20u128), Uint256::from(100u128)),
            pending_rewards: Decimal256::from_ratio(Uint256::from(20u128), 1u128)
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user1", "cw20:incentive_token"))
            .unwrap()
    );

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::from_ratio(Uint256::from(40u128), Uint256::from(100u128)),
            pending_rewards: Decimal256::from_ratio(Uint256::from(40u128), 1u128)
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user1", "native:uusd"))
            .unwrap()
    );

    assert_eq!(
        StakerRewardInfo {
            // Halfway through cycle -> half of rewards available
            index: Decimal256::from_ratio(Uint256::from(50u128), Uint256::from(100u128)),
            pending_rewards: Decimal256::from_ratio(Uint256::from(50u128), 1u128)
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user1", "cw20:allocated_token"))
            .unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(50u128), Uint256::from(100u128)),
            asset_info: AssetInfo::cw20(Addr::unchecked(MOCK_ALLOCATED_TOKEN)),
            reward_type: RewardType::Allocated {
                init_timestamp: current_timestamp,

                allocation_config: AllocationConfig {
                    till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(100)),
                    cycle_rewards: Uint128::new(100),
                    cycle_duration: Milliseconds::from_seconds(100),
                    reward_increase: None,
                },
                allocation_state: AllocationState {
                    current_cycle: 0,
                    current_cycle_rewards: Uint128::new(100),
                    last_distributed: current_timestamp.plus_seconds(50),
                },
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "cw20:allocated_token")
            .unwrap()
    );
}

#[test]
fn test_add_reward_token() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(deps.as_mut(), None).unwrap();

    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
        (
            &MOCK_INCENTIVE_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::zero())],
        ),
    ]);

    STATE
        .save(
            deps.as_mut().storage,
            &State {
                total_share: Uint128::new(100),
            },
        )
        .unwrap();

    let msg = ExecuteMsg::AddRewardToken {
        reward_token: RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        },
    };
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "add_reward_token")
            .add_attribute("added_token", "cw20:incentive_token"),
        res
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::zero(),
            asset_info: AssetInfo::cw20(Addr::unchecked(MOCK_INCENTIVE_TOKEN)),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::zero(),
                init_timestamp: current_timestamp,
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "cw20:incentive_token")
            .unwrap()
    );
}

#[test]
fn test_add_reward_token_duplicate() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::native("uusd"),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        }]),
    )
    .unwrap();

    let msg = ExecuteMsg::AddRewardToken {
        reward_token: RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::native("uusd"),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        },
    };
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), mock_env(), info, msg);
    assert_eq!(
        ContractError::InvalidAsset {
            asset: "native:uusd".to_string()
        },
        res.unwrap_err()
    );
}

#[test]
fn test_add_reward_token_staking_token() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(deps.as_mut(), None).unwrap();

    let msg = ExecuteMsg::AddRewardToken {
        reward_token: RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_STAKING_TOKEN),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        },
    };
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), mock_env(), info, msg);
    assert_eq!(
        ContractError::InvalidAsset {
            asset: "cw20:staking_token".to_string()
        },
        res.unwrap_err()
    );
}

#[test]
fn test_add_reward_token_unauthorized() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(deps.as_mut(), None).unwrap();

    let msg = ExecuteMsg::AddRewardToken {
        reward_token: RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_STAKING_TOKEN),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        },
    };
    let info = mock_info("not_owner", &[]);

    let res = execute(deps.as_mut(), mock_env(), info, msg);
    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
}

#[test]
fn test_add_reward_token_exceeds_max() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    let mut reward_tokens: Vec<RewardTokenUnchecked> = vec![];

    for i in 0..MAX_REWARD_TOKENS {
        reward_tokens.push(RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(format!("token{i}")),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        });
    }

    let _res = init(deps.as_mut(), Some(reward_tokens)).unwrap();

    let msg = ExecuteMsg::AddRewardToken {
        reward_token: RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        },
    };
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        ContractError::MaxRewardTokensExceeded {
            max: MAX_REWARD_TOKENS
        },
        res.unwrap_err()
    );
}

#[test]
fn test_remove_reward_token() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::native("uusd"),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        }]),
    )
    .unwrap();

    let msg = ExecuteMsg::RemoveRewardToken {
        reward_token: "native:uusd".to_string(),
    };
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "remove_reward_token")
            .add_attribute("number_of_reward_tokens", "0")
            .add_attribute("removed_token", "native:uusd"),
        res
    );

    let reward_token = REWARD_TOKENS
        .load(deps.as_ref().storage, "native:uusd")
        .unwrap();
    assert!(!reward_token.is_active);
}

#[test]
fn test_remove_reward_token_unauthorized() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::native("uusd"),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        }]),
    )
    .unwrap();

    let msg = ExecuteMsg::RemoveRewardToken {
        reward_token: "native:uusd".to_string(),
    };
    let info = mock_info("owner1", &[]);

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
}

#[test]
fn test_remove_reward_token_invalid_asset() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        }]),
    )
    .unwrap();

    let msg = ExecuteMsg::RemoveRewardToken {
        reward_token: "native:uusd".to_string(),
    };
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        ContractError::InvalidAsset {
            asset: "native:uusd".to_string()
        },
        res.unwrap_err()
    );
}

#[test]
fn test_claim_rewards_after_remove() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());

    // Init with additional rewards
    init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::native("uusd"),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        }]),
    )
    .unwrap();

    deps.querier.with_token_balances(&[(
        &MOCK_STAKING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100 + 100))],
    )]);

    // user1 and user2 stake with 100 tokens separately
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user1".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    deps.querier.with_token_balances(&[(
        &MOCK_STAKING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(200 + 100))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user2".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Staker {
            share: Uint128::new(100)
        },
        STAKERS.load(deps.as_ref().storage, "user1").unwrap()
    );
    assert_eq!(
        Staker {
            share: Uint128::new(50)
        },
        STAKERS.load(deps.as_ref().storage, "user2").unwrap()
    );

    let info = mock_info("user1", &[]);
    let msg = ExecuteMsg::ClaimRewards {};
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    // No rewards have been given yet.
    assert_eq!(ContractError::WithdrawalIsEmpty {}, res.unwrap_err());

    deps.querier
        .base
        .update_balance(mock_env().contract.address, coins(100, "uusd"));

    // Update the global index for uusd by depositing 100 uusd
    let msg = ExecuteMsg::UpdateGlobalIndexes {
        asset_infos: Some(vec![AssetInfoUnchecked::native("uusd")]),
    };

    let info = mock_info("owner", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(100u128), Uint256::from(150u128)),
            asset_info: AssetInfo::native("uusd"),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::new(100),
                init_timestamp: current_timestamp,
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "native:uusd")
            .unwrap()
    );

    let info = mock_info("user1", &[]);
    let msg = ExecuteMsg::ClaimRewards {};
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::from_ratio(Uint256::from(100u128), Uint256::from(150u128)),
            pending_rewards: Decimal256::zero(),
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user1", "native:uusd"))
            .unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(100u128), Uint256::from(150u128)),
            asset_info: AssetInfo::native("uusd"),
            reward_type: RewardType::NonAllocated {
                previous_reward_balance: Uint128::new(34),
                init_timestamp: current_timestamp,
            },
            is_active: true,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "native:uusd")
            .unwrap()
    );

    assert_eq!(
        Response::new()
            .add_attribute("action", "claim_rewards")
            .add_message(BankMsg::Send {
                to_address: "user1".to_string(),
                amount: coins(66, "uusd")
            }),
        res
    );

    deps.querier
        .base
        .update_balance(mock_env().contract.address, coins(34, "uusd"));

    let msg = ExecuteMsg::RemoveRewardToken {
        reward_token: "native:uusd".to_string(),
    };
    let info = mock_info("owner", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let info = mock_info("user2", &[]);
    let msg = ExecuteMsg::ClaimRewards {};
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "claim_rewards")
            .add_message(BankMsg::Send {
                to_address: "user2".to_string(),
                amount: coins(33, "uusd")
            }),
        res
    );

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::from_ratio(Uint256::from(100u128), Uint256::from(150u128)),
            pending_rewards: Decimal256::zero()
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user2", "native:uusd"))
            .unwrap()
    );

    // Last reward is distributed and reward token is removed from the reward token list
    assert!(!REWARD_TOKENS.has(deps.as_ref().storage, "native:uusd"));
    deps.querier
        .base
        .update_balance(mock_env().contract.address, coins(1, "uusd"));

    // Verify that the queries return the empty pending rewards.
    let msg = QueryMsg::Stakers {
        start_after: None,
        limit: None,
    };
    let res: Vec<StakerResponse> =
        from_json(query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

    assert_eq!(
        vec![
            StakerResponse {
                address: "user1".to_string(),
                share: Uint128::new(100),
                pending_rewards: vec![],
                balance: Uint128::new(200),
            },
            StakerResponse {
                address: "user2".to_string(),
                share: Uint128::new(50),
                pending_rewards: vec![],
                balance: Uint128::new(100),
            },
        ],
        res
    );
}

#[test]
fn test_claim_rewards_allocated_after_remove() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_ALLOCATED_TOKEN),
            init_timestamp: Expiry::AtTime(current_timestamp),
            allocation_config: Some(AllocationConfig {
                till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(100)),
                cycle_rewards: Uint128::new(100),
                cycle_duration: Milliseconds::from_seconds(100),
                reward_increase: None,
            }),
        }]),
    )
    .unwrap();

    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            // 100 is user's deposit.
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
        (
            &MOCK_ALLOCATED_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
    ]);

    // user stake with 100 mock token
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "user".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::StakeTokens {}).unwrap(),
    });

    let info = mock_info(MOCK_STAKING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    deps.querier.with_token_balances(&[
        (
            &MOCK_STAKING_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100 + 100))],
        ),
        (
            &MOCK_ALLOCATED_TOKEN.to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100))],
        ),
    ]);

    // Speed time up to halfway through cycle.
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(50);

    // User claims rewards.
    let info = mock_info("user", &[]);
    let msg = ExecuteMsg::ClaimRewards {};
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let msg = ExecuteMsg::RemoveRewardToken {
        reward_token: format!("cw20:{MOCK_ALLOCATED_TOKEN}"),
    };

    env.block.time = env.block.time.plus_seconds(25);
    let info = mock_info("owner", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    env.block.time = env.block.time.plus_seconds(25);
    let info = mock_info("user", &[]);
    let msg = ExecuteMsg::ClaimRewards {};
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "claim_rewards")
            .add_message(WasmMsg::Execute {
                contract_addr: MOCK_ALLOCATED_TOKEN.to_owned(),
                funds: vec![],
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "user".to_string(),
                    amount: Uint128::new(25)
                })
                .unwrap(),
            }),
        res
    );

    assert_eq!(
        StakerRewardInfo {
            index: Decimal256::percent(75),
            pending_rewards: Decimal256::zero(),
        },
        STAKER_REWARD_INFOS
            .load(deps.as_ref().storage, ("user", "cw20:allocated_token"))
            .unwrap()
    );

    assert_eq!(
        RewardToken {
            index: Decimal256::from_ratio(Uint256::from(150u128), Uint256::from(200u128)),
            asset_info: AssetInfo::cw20(Addr::unchecked(MOCK_ALLOCATED_TOKEN)),
            reward_type: RewardType::Allocated {
                init_timestamp: current_timestamp,
                allocation_config: AllocationConfig {
                    till_timestamp: Expiry::AtTime(current_timestamp.plus_seconds(100)),
                    cycle_rewards: Uint128::new(100),
                    cycle_duration: Milliseconds::from_seconds(100),
                    reward_increase: None,
                },
                allocation_state: AllocationState {
                    current_cycle: 0,
                    current_cycle_rewards: Uint128::new(100),
                    last_distributed: current_timestamp.plus_seconds(75),
                },
            },
            is_active: false,
        },
        REWARD_TOKENS
            .load(deps.as_ref().storage, "cw20:allocated_token")
            .unwrap()
    );
}

#[test]
fn test_replace_reward_token() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::native("uusd"),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        }]),
    )
    .unwrap();

    let msg = ExecuteMsg::ReplaceRewardToken {
        origin_reward_token: "native:uusd".to_string(),
        reward_token: RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        },
    };
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "replace_reward_token")
            .add_attribute("origin_reward_token", "native:uusd")
            .add_attribute("new_reward_token", format!("cw20:{MOCK_INCENTIVE_TOKEN}")),
        res
    );

    let reward_token = REWARD_TOKENS
        .load(deps.as_ref().storage, "native:uusd")
        .unwrap();
    assert!(!reward_token.is_active);

    assert!(REWARD_TOKENS.has(
        deps.as_ref().storage,
        &format!("cw20:{MOCK_INCENTIVE_TOKEN}")
    ));
}
#[test]
fn test_replace_reward_token_unauthorized() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::native("uusd"),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        }]),
    )
    .unwrap();

    let msg = ExecuteMsg::ReplaceRewardToken {
        origin_reward_token: "native:uusd".to_string(),
        reward_token: RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        },
    };
    let info = mock_info("owner1", &[]);

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
}

#[test]
fn test_replace_reward_token_invalid_asset() {
    let mut deps = mock_dependencies_custom(&[]);
    let current_timestamp = Milliseconds::from_seconds(mock_env().block.time.seconds());
    init(
        deps.as_mut(),
        Some(vec![RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::native("uusd"),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        }]),
    )
    .unwrap();

    let msg = ExecuteMsg::ReplaceRewardToken {
        origin_reward_token: "native:uusd".to_string(),
        reward_token: RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::native("uusd"),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        },
    };
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        ContractError::InvalidAsset {
            asset: "native:uusd".to_string()
        },
        res.unwrap_err()
    );

    let msg = ExecuteMsg::ReplaceRewardToken {
        origin_reward_token: "cw20:uusd".to_string(),
        reward_token: RewardTokenUnchecked {
            asset_info: AssetInfoUnchecked::cw20(MOCK_INCENTIVE_TOKEN),
            allocation_config: None,
            init_timestamp: Expiry::AtTime(current_timestamp),
        },
    };
    let info = mock_info("owner", &[]);

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        ContractError::InvalidAsset {
            asset: "cw20:uusd".to_string()
        },
        res.unwrap_err()
    );
}
