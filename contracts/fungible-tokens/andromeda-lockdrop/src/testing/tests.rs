use crate::state::{State, UserInfo, USER_INFO};
use crate::testing::mock_querier::mock_dependencies_custom;
use crate::{
    contract::{execute, instantiate, query},
    state::{CONFIG, STATE},
};
use andromeda_fungible_tokens::lockdrop::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, StateResponse,
    UserInfoResponse,
};
use andromeda_std::amp::AndrAddr;
use andromeda_std::common::expiration::Expiry;
use andromeda_std::{
    common::{expiration::MILLISECONDS_TO_NANOSECONDS_RATIO, Milliseconds},
    error::ContractError,
    testing::mock_querier::MOCK_KERNEL_CONTRACT,
};
use cosmwasm_std::{
    coin, coins, from_json,
    testing::{mock_env, mock_info},
    to_json_binary, Addr, BankMsg, Decimal, DepsMut, Response, Uint128, WasmMsg,
};

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

const MOCK_INCENTIVE_TOKEN: &str = "mock_incentive_token";
const DEPOSIT_WINDOW: u64 = 5;
const WITHDRAWAL_WINDOW: u64 = 4;

fn init(deps: DepsMut) -> Result<Response, ContractError> {
    let env = mock_env();
    let info = mock_info("owner", &[]);

    let msg = InstantiateMsg {
        // bootstrap_contract: None,
        init_timestamp: Expiry::AtTime(Milliseconds::from_nanos(env.block.time.nanos())),
        deposit_window: Milliseconds::from_seconds(DEPOSIT_WINDOW),
        withdrawal_window: Milliseconds::from_seconds(WITHDRAWAL_WINDOW),
        incentive_token: AndrAddr::from_string(MOCK_INCENTIVE_TOKEN),
        native_denom: "uusd".to_string(),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    instantiate(deps, env, info, msg)
}

#[test]
fn test_instantiate() {
    let mut deps = mock_dependencies_custom(&[]);

    let res = init(deps.as_mut()).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("type", "lockdrop")
            .add_attribute("kernel_address", MOCK_KERNEL_CONTRACT)
            .add_attribute("owner", "owner"),
        res
    );

    let msg = QueryMsg::Config {};
    let config_res: ConfigResponse =
        from_json(query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

    assert_eq!(
        ConfigResponse {
            // bootstrap_contract_address: None,
            init_timestamp: Milliseconds::from_nanos(mock_env().block.time.nanos()),
            deposit_window: Milliseconds::from_seconds(DEPOSIT_WINDOW),
            withdrawal_window: Milliseconds::from_seconds(WITHDRAWAL_WINDOW),
            lockdrop_incentives: Uint128::zero(),
            incentive_token: AndrAddr::from_string(MOCK_INCENTIVE_TOKEN),
            native_denom: "uusd".to_string()
        },
        config_res
    );

    let msg = QueryMsg::State {};
    let state_res: StateResponse =
        from_json(query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

    assert_eq!(
        StateResponse {
            total_native_locked: Uint128::zero(),
            are_claims_allowed: false,
        },
        state_res
    );
}

#[test]
fn test_instantiate_init_timestamp_past() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("owner", &[]);

    let msg = InstantiateMsg {
        // bootstrap_contract: None,
        init_timestamp: Expiry::AtTime(Milliseconds::from_seconds(env.block.time.seconds() - 1)),
        deposit_window: Milliseconds::from_seconds(5),
        withdrawal_window: Milliseconds::from_seconds(2),
        incentive_token: AndrAddr::from_string(MOCK_INCENTIVE_TOKEN),
        native_denom: "uusd".to_string(),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let res = instantiate(deps.as_mut(), env.clone(), info, msg);

    assert_eq!(
        ContractError::StartTimeInThePast {
            current_time: env.block.time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO,
            current_block: env.block.height,
        },
        res.unwrap_err()
    );
}

#[test]
fn test_instantiate_init_deposit_window_zero() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("owner", &[]);

    let msg = InstantiateMsg {
        // bootstrap_contract: None,
        init_timestamp: Expiry::AtTime(Milliseconds::from_seconds(env.block.time.seconds() + 1)),
        deposit_window: Milliseconds::from_seconds(0),
        withdrawal_window: Milliseconds::from_seconds(2),
        incentive_token: AndrAddr::from_string(MOCK_INCENTIVE_TOKEN),
        native_denom: "uusd".to_string(),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let res = instantiate(deps.as_mut(), env, info, msg);

    assert_eq!(ContractError::InvalidWindow {}, res.unwrap_err());
}

#[test]
fn test_instantiate_init_withdrawal_window_zero() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("owner", &[]);

    let msg = InstantiateMsg {
        // bootstrap_contract: None,
        init_timestamp: Expiry::AtTime(Milliseconds::from_seconds(env.block.time.seconds() + 1)),
        deposit_window: Milliseconds::from_seconds(5),
        withdrawal_window: Milliseconds::from_seconds(0),
        incentive_token: AndrAddr::from_string(MOCK_INCENTIVE_TOKEN),
        native_denom: "uusd".to_string(),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let res = instantiate(deps.as_mut(), env, info, msg);

    assert_eq!(ContractError::InvalidWindow {}, res.unwrap_err());
}

#[test]
fn test_instantiate_init_deposit_window_less_than_withdrawal_window() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("owner", &[]);

    let msg = InstantiateMsg {
        // bootstrap_contract: None,
        init_timestamp: Expiry::AtTime(Milliseconds::from_seconds(env.block.time.seconds() + 1)),
        deposit_window: Milliseconds::from_seconds(2),
        withdrawal_window: Milliseconds::from_seconds(5),
        incentive_token: AndrAddr::from_string(MOCK_INCENTIVE_TOKEN),
        native_denom: "uusd".to_string(),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let res = instantiate(deps.as_mut(), env, info, msg);

    assert_eq!(ContractError::InvalidWindow {}, res.unwrap_err());
}

#[test]
fn test_increase_incentives() {
    let mut deps = mock_dependencies_custom(&[]);

    init(deps.as_mut()).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "owner".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::IncreaseIncentives {}).unwrap(),
    });

    let info = mock_info(MOCK_INCENTIVE_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "incentives_increased")
            .add_attribute("amount", "100"),
        res
    );

    assert_eq!(
        Uint128::new(100),
        CONFIG
            .load(deps.as_ref().storage)
            .unwrap()
            .lockdrop_incentives
    );
}

#[test]
fn test_increase_incentives_invalid_token() {
    let mut deps = mock_dependencies_custom(&[]);

    init(deps.as_mut()).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "owner".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::IncreaseIncentives {}).unwrap(),
    });

    let info = mock_info("invalid_token", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    assert_eq!(
        ContractError::InvalidFunds {
            msg: "Only incentive tokens are valid".to_string(),
        },
        res.unwrap_err()
    );
}

#[test]
fn test_increase_incentives_after_phase_ends() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();

    init(deps.as_mut()).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "owner".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::IncreaseIncentives {}).unwrap(),
    });

    env.block.time = env
        .block
        .time
        .plus_seconds(DEPOSIT_WINDOW + WITHDRAWAL_WINDOW + 1);
    let info = mock_info(MOCK_INCENTIVE_TOKEN, &[]);
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(
        ContractError::TokenAlreadyBeingDistributed {},
        res.unwrap_err()
    );
}

#[test]
fn test_increase_incentives_zero_amount() {
    let mut deps = mock_dependencies_custom(&[]);

    init(deps.as_mut()).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "owner".to_string(),
        amount: Uint128::zero(),
        msg: to_json_binary(&Cw20HookMsg::IncreaseIncentives {}).unwrap(),
    });

    let info = mock_info(MOCK_INCENTIVE_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        ContractError::InvalidFunds {
            msg: "Number of tokens should be > 0".to_string(),
        },
        res.unwrap_err()
    );
}

#[test]
fn test_deposit_native() {
    let mut deps = mock_dependencies_custom(&[]);
    init(deps.as_mut()).unwrap();

    let msg = ExecuteMsg::DepositNative {};
    let info = mock_info("sender", &coins(100, "uusd"));

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "lock_native")
            .add_attribute("user", "sender")
            .add_attribute("ust_deposited", "100"),
        res
    );

    assert_eq!(
        State {
            total_native_locked: Uint128::new(100),
            total_delegated: Uint128::zero(),
            are_claims_allowed: false
        },
        STATE.load(deps.as_ref().storage,).unwrap()
    );

    assert_eq!(
        UserInfo {
            total_native_locked: Uint128::new(100),
            delegated_incentives: Uint128::zero(),
            lockdrop_claimed: false,
            withdrawal_flag: false,
        },
        USER_INFO
            .load(deps.as_ref().storage, &Addr::unchecked("sender"))
            .unwrap()
    );
}

#[test]
fn test_deposit_native_zero_amount() {
    let mut deps = mock_dependencies_custom(&[]);
    init(deps.as_mut()).unwrap();

    let msg = ExecuteMsg::DepositNative {};
    let info = mock_info("sender", &coins(0, "uusd"));

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        ContractError::InvalidFunds {
            msg: "Amount must be greater than 0".to_string(),
        },
        res.unwrap_err()
    );
}

#[test]
fn test_deposit_native_wrong_denom() {
    let mut deps = mock_dependencies_custom(&[]);
    init(deps.as_mut()).unwrap();

    let msg = ExecuteMsg::DepositNative {};
    let info = mock_info("sender", &coins(100, "uluna"));

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        ContractError::InvalidFunds {
            msg: "Only uusd accepted".to_string(),
        },
        res.unwrap_err()
    );
}

#[test]
fn test_deposit_native_multiple_denoms() {
    let mut deps = mock_dependencies_custom(&[]);
    init(deps.as_mut()).unwrap();

    let msg = ExecuteMsg::DepositNative {};
    let info = mock_info("sender", &[coin(100, "uluna"), coin(100, "uusd")]);

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        ContractError::InvalidFunds {
            msg: "Must deposit a single fund".to_string(),
        },
        res.unwrap_err()
    );
}

#[test]
fn test_deposit_native_deposit_window_closed() {
    let mut deps = mock_dependencies_custom(&[]);
    init(deps.as_mut()).unwrap();

    let msg = ExecuteMsg::DepositNative {};
    let info = mock_info("sender", &coins(100, "uusd"));

    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(DEPOSIT_WINDOW + 1);
    let res = execute(deps.as_mut(), env, info, msg);

    assert_eq!(ContractError::DepositWindowClosed {}, res.unwrap_err());
}

#[test]
fn test_withdraw_native() {
    let mut deps = mock_dependencies_custom(&[]);
    init(deps.as_mut()).unwrap();

    let msg = ExecuteMsg::DepositNative {};
    let info = mock_info("sender", &coins(100, "uusd"));

    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::WithdrawNative { amount: None };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_message(BankMsg::Send {
                to_address: "sender".to_string(),
                amount: coins(100, "uusd")
            })
            .add_attribute("action", "withdraw_native")
            .add_attribute("user", "sender")
            .add_attribute("amount", "100"),
        res
    );

    assert_eq!(
        State {
            total_native_locked: Uint128::zero(),
            total_delegated: Uint128::zero(),
            are_claims_allowed: false
        },
        STATE.load(deps.as_ref().storage,).unwrap()
    );

    assert_eq!(
        UserInfo {
            total_native_locked: Uint128::zero(),
            delegated_incentives: Uint128::zero(),
            lockdrop_claimed: false,
            withdrawal_flag: false,
        },
        USER_INFO
            .load(deps.as_ref().storage, &Addr::unchecked("sender"))
            .unwrap()
    );
}

// #[test]
// fn test_withdraw_native_withdraw_phase_first_half() {
//     let mut deps = mock_dependencies_custom(&[]);
//     init(deps.as_mut()).unwrap();

//     let msg = ExecuteMsg::DepositNative {};
//     let info = mock_info("sender", &coins(100, "uusd"));

//     let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

//     let msg = ExecuteMsg::WithdrawNative {
//         amount: Some(Uint128::new(51)),
//     };

//     let mut env = mock_env();
//     env.block.time = env.block.time.plus_seconds(DEPOSIT_WINDOW + 1);
//     let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

//     assert_eq!(
//         ContractError::InvalidWithdrawal {
//             msg: Some("Amount exceeds max allowed withdrawal limit of 50".to_string()),
//         },
//         res.unwrap_err()
//     );

//     let msg = ExecuteMsg::WithdrawNative { amount: None };

//     let res = execute(deps.as_mut(), env, info, msg).unwrap();

//     assert_eq!(
//         Response::new()
//             .add_message(BankMsg::Send {
//                 to_address: "sender".to_string(),
//                 amount: coins(50, "uusd")
//             })
//             .add_attribute("action", "withdraw_native")
//             .add_attribute("user", "sender")
//             // Only half is withdrawable in the first half of the withdrawal period
//             .add_attribute("amount", "50"),
//         res
//     );

//     assert_eq!(
//         State {
//             total_native_locked: Uint128::new(50),
//             total_delegated: Uint128::zero(),
//             are_claims_allowed: false
//         },
//         STATE.load(deps.as_ref().storage,).unwrap()
//     );

//     assert_eq!(
//         UserInfo {
//             total_native_locked: Uint128::new(50),
//             delegated_incentives: Uint128::zero(),
//             lockdrop_claimed: false,
//             withdrawal_flag: true,
//         },
//         USER_INFO
//             .load(deps.as_ref().storage, &Addr::unchecked("sender"))
//             .unwrap()
//     );
// }

// #[test]
// fn test_withdraw_native_withdraw_phase_second_half() {
//     let mut deps = mock_dependencies_custom(&[]);
//     init(deps.as_mut()).unwrap();

//     let msg = ExecuteMsg::DepositNative {};
//     let info = mock_info("sender", &coins(100, "uusd"));

//     let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

//     let msg = ExecuteMsg::WithdrawNative { amount: None };

//     let mut env = mock_env();
//     env.block.time = env
//         .block
//         .time
//         .plus_seconds(DEPOSIT_WINDOW + 3 * WITHDRAWAL_WINDOW / 4);
//     let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

//     assert_eq!(
//         Response::new()
//             .add_message(BankMsg::Send {
//                 to_address: "sender".to_string(),
//                 amount: coins(100, "uusd")
//             })
//             .add_attribute("action", "withdraw_native")
//             .add_attribute("user", "sender")
//             // In second half of withdrawal phase, percent decreases linearly from 50% to 0%.
//             .add_attribute("amount", "25"),
//         res
//     );

//     assert_eq!(
//         State {
//             total_native_locked: Uint128::new(75),
//             total_delegated: Uint128::zero(),
//             are_claims_allowed: false
//         },
//         STATE.load(deps.as_ref().storage).unwrap()
//     );

//     assert_eq!(
//         UserInfo {
//             total_native_locked: Uint128::new(75),
//             delegated_incentives: Uint128::zero(),
//             lockdrop_claimed: false,
//             withdrawal_flag: true,
//         },
//         USER_INFO
//             .load(deps.as_ref().storage, &Addr::unchecked("sender"))
//             .unwrap()
//     );

//     // try to withdraw again
//     let res = execute(deps.as_mut(), env, info, msg);

//     assert_eq!(
//         ContractError::InvalidWithdrawal {
//             msg: Some("Max 1 withdrawal allowed".to_string()),
//         },
//         res.unwrap_err()
//     );
// }

// #[test]
// fn test_withdraw_proceeds_unauthorized() {
//     let mut deps = mock_dependencies_custom(&[]);
//     init(deps.as_mut()).unwrap();

//     let msg = ExecuteMsg::WithdrawProceeds { recipient: None };

//     let info = mock_info("not owner", &[]);
//     let res = execute(deps.as_mut(), mock_env(), info, msg);

//     assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
// }

// #[test]
// fn test_withdraw_proceeds_phase_not_started() {
//     let mut deps = mock_dependencies_custom(&[]);
//     init(deps.as_mut()).unwrap();

//     let msg = ExecuteMsg::WithdrawProceeds { recipient: None };

//     let info = mock_info("owner", &[]);
//     let mut env = mock_env();
//     env.block.time = env.block.time.minus_seconds(1);
//     let res = execute(deps.as_mut(), env, info, msg);

//     assert_eq!(
//         ContractError::InvalidWithdrawal {
//             msg: Some("Lockdrop withdrawals haven't concluded yet".to_string()),
//         },
//         res.unwrap_err()
//     );
// }

// #[test]
// fn test_withdraw_proceeds_phase_not_ended() {
//     let mut deps = mock_dependencies_custom(&[]);
//     init(deps.as_mut()).unwrap();

//     let msg = ExecuteMsg::WithdrawProceeds { recipient: None };

//     let info = mock_info("owner", &[]);
//     let mut env = mock_env();
//     env.block.time = env.block.time.plus_seconds(DEPOSIT_WINDOW);
//     let res = execute(deps.as_mut(), mock_env(), info, msg);

//     assert_eq!(
//         ContractError::InvalidWithdrawal {
//             msg: Some("Lockdrop withdrawals haven't concluded yet".to_string()),
//         },
//         res.unwrap_err()
//     );
// }

// #[test]
// fn test_withdraw_proceeds() {
//     // This uusd is to simulate the deposit made prior to withdrawing proceeds. This is needed
//     // since the mock querier doesn't automatically assign balances.
//     let amount = 100;
//     let mut deps = mock_dependencies_custom(&[coin(amount, "uusd")]);
//     init(deps.as_mut()).unwrap();

//     let msg = ExecuteMsg::DepositNative {};
//     let info = mock_info("sender", &coins(amount, "uusd"));

//     let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

//     // Update contract's balance after deposit
//     deps.querier
//         .base
//         .update_balance(MOCK_CONTRACT_ADDR, coins(amount, "uusd"));

//     let msg = ExecuteMsg::WithdrawProceeds { recipient: None };

//     let info = mock_info("owner", &[]);
//     let mut env = mock_env();
//     env.block.time = env
//         .block
//         .time
//         .plus_seconds(DEPOSIT_WINDOW + WITHDRAWAL_WINDOW + 1);

//     let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

//     assert_eq!(
//         Response::new()
//             .add_message(BankMsg::Send {
//                 to_address: "owner".to_string(),
//                 amount: coins(100, "uusd")
//             })
//             .add_attribute("action", "withdraw_proceeds")
//             .add_attribute("amount", "100")
//             .add_attribute("timestamp", env.block.time.seconds().to_string()),
//         res
//     );

//     // Remove withdrawn funds.
//     deps.querier
//         .base
//         .update_balance(env.contract.address.clone(), vec![]);

//     // try to withdraw again
//     let res = execute(deps.as_mut(), env, info, msg);

//     assert_eq!(
//         ContractError::InvalidWithdrawal {
//             msg: Some("Already withdrew funds".to_string()),
//         },
//         res.unwrap_err()
//     );
// }

#[test]
fn test_enable_claims_no_bootstrap_specified() {
    let mut deps = mock_dependencies_custom(&[]);
    init(deps.as_mut()).unwrap();

    let msg = ExecuteMsg::EnableClaims {};

    let mut env = mock_env();
    env.block.time = env
        .block
        .time
        .plus_seconds(DEPOSIT_WINDOW + WITHDRAWAL_WINDOW + 1);

    let info = mock_info("sender", &[]);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

    assert_eq!(
        Response::new().add_attribute("action", "enable_claims"),
        res
    );

    assert_eq!(
        State {
            total_delegated: Uint128::zero(),
            total_native_locked: Uint128::zero(),
            are_claims_allowed: true
        },
        STATE.load(deps.as_ref().storage).unwrap()
    );

    // Try to do it again.
    let res = execute(deps.as_mut(), env, info, msg);

    assert_eq!(ContractError::ClaimsAlreadyAllowed {}, res.unwrap_err());
}

// #[test]
// fn test_enable_claims_bootstrap_specified() {
//     let mut deps = mock_dependencies_custom(&[]);
//     let msg = InstantiateMsg {
//         // bootstrap_contract: Some(AndrAddress {
//         //     identifier: MOCK_BOOTSTRAP_CONTRACT.to_owned(),
//         // }),
//         init_timestamp: mock_env().block.time.seconds(),
//         deposit_window: DEPOSIT_WINDOW,
//         withdrawal_window: WITHDRAWAL_WINDOW,
//         incentive_token: AndrAddr::from_string(MOCK_INCENTIVE_TOKEN),
//         native_denom: "uusd".to_string(),
//     };

//     let info = mock_info("owner", &[]);
//     let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

//     let msg = ExecuteMsg::EnableClaims {};

//     let mut env = mock_env();
//     env.block.time = env
//         .block
//         .time
//         .plus_seconds(DEPOSIT_WINDOW + WITHDRAWAL_WINDOW + 1);

//     let info = mock_info("not_bootstrap_contract", &[]);
//     let res = execute(deps.as_mut(), env.clone(), info, msg.clone());

//     assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());

//     let info = mock_info(MOCK_BOOTSTRAP_CONTRACT, &[]);
//     let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

//     assert_eq!(
//         Response::new().add_attribute("action", "enable_claims"),
//         res
//     );

//     assert_eq!(
//         State {
//             total_delegated: Uint128::zero(),
//             total_native_locked: Uint128::zero(),
//             are_claims_allowed: true
//         },
//         STATE.load(deps.as_ref().storage).unwrap()
//     );

//     // Try to do it again.
//     let res = execute(deps.as_mut(), env, info, msg);

//     assert_eq!(ContractError::ClaimsAlreadyAllowed {}, res.unwrap_err());
// }

#[test]
fn test_enable_claims_phase_not_ended() {
    let mut deps = mock_dependencies_custom(&[]);
    init(deps.as_mut()).unwrap();

    let msg = ExecuteMsg::EnableClaims {};

    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(DEPOSIT_WINDOW - 1);

    let info = mock_info("sender", &[]);
    let res = execute(deps.as_mut(), env, info, msg);

    assert_eq!(ContractError::PhaseOngoing {}, res.unwrap_err());
}

#[test]
fn test_claim_rewards() {
    let mut deps = mock_dependencies_custom(&[]);
    init(deps.as_mut()).unwrap();

    // First increase incentives
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "owner".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::IncreaseIncentives {}).unwrap(),
    });

    let info = mock_info(MOCK_INCENTIVE_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Then User1 deposits
    let msg = ExecuteMsg::DepositNative {};
    let info = mock_info("user1", &coins(75, "uusd"));

    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Then User2 deposits
    let msg = ExecuteMsg::DepositNative {};
    let info = mock_info("user2", &coins(25, "uusd"));

    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        State {
            total_native_locked: Uint128::new(100),
            total_delegated: Uint128::zero(),
            are_claims_allowed: false
        },
        STATE.load(deps.as_ref().storage).unwrap()
    );

    // Skip time to end of phase
    let mut env = mock_env();
    env.block.time = env
        .block
        .time
        .plus_seconds(DEPOSIT_WINDOW + WITHDRAWAL_WINDOW + 1);

    // Enable claims
    let msg = ExecuteMsg::EnableClaims {};

    let info = mock_info("sender", &[]);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // User 1 claims rewards
    let msg = ExecuteMsg::ClaimRewards {};
    let info = mock_info("user1", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "claim_rewards")
            .add_attribute("amount", "75")
            .add_message(WasmMsg::Execute {
                contract_addr: MOCK_INCENTIVE_TOKEN.to_string(),
                funds: vec![],
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "user1".to_string(),
                    amount: Uint128::new(75)
                })
                .unwrap()
            }),
        res
    );

    let msg = QueryMsg::UserInfo {
        address: "user1".to_string(),
    };
    let user_res: UserInfoResponse =
        from_json(query(deps.as_ref(), env.clone(), msg).unwrap()).unwrap();

    assert_eq!(
        UserInfoResponse {
            total_native_locked: Uint128::new(75),
            is_lockdrop_claimed: true,
            withdrawal_flag: false,
            total_incentives: Uint128::new(75),
        },
        user_res
    );

    // User 2 claims rewards
    let msg = ExecuteMsg::ClaimRewards {};
    let info = mock_info("user2", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "claim_rewards")
            .add_attribute("amount", "25")
            .add_message(WasmMsg::Execute {
                contract_addr: MOCK_INCENTIVE_TOKEN.to_string(),
                funds: vec![],
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "user2".to_string(),
                    amount: Uint128::new(25)
                })
                .unwrap()
            }),
        res
    );

    let msg = QueryMsg::UserInfo {
        address: "user2".to_string(),
    };
    let user_res: UserInfoResponse =
        from_json(query(deps.as_ref(), env.clone(), msg).unwrap()).unwrap();

    assert_eq!(
        UserInfoResponse {
            total_native_locked: Uint128::new(25),
            is_lockdrop_claimed: true,
            withdrawal_flag: false,
            total_incentives: Uint128::new(25),
        },
        user_res
    );

    // User 3 tries to claim rewards
    let msg = ExecuteMsg::ClaimRewards {};
    let info = mock_info("user3", &[]);

    let res = execute(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(ContractError::NoLockup {}, res.unwrap_err());

    // User 2 tries to claim again
    let msg = ExecuteMsg::ClaimRewards {};
    let info = mock_info("user2", &[]);

    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::LockdropAlreadyClaimed {}, res.unwrap_err());
}

#[test]
fn test_claim_rewards_not_available() {
    let mut deps = mock_dependencies_custom(&[]);
    init(deps.as_mut()).unwrap();

    // First increase incentives
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "owner".to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&Cw20HookMsg::IncreaseIncentives {}).unwrap(),
    });

    let info = mock_info(MOCK_INCENTIVE_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Then User1 deposits
    let msg = ExecuteMsg::DepositNative {};
    let info = mock_info("user1", &coins(75, "uusd"));

    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // Try to claim rewards
    let msg = ExecuteMsg::ClaimRewards {};
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(ContractError::ClaimsNotAllowed {}, res.unwrap_err());
}

#[test]
fn test_query_withdrawable_percent() {
    let mut deps = mock_dependencies_custom(&[]);
    init(deps.as_mut()).unwrap();

    let msg = QueryMsg::WithdrawalPercentAllowed { timestamp: None };
    let res: Decimal = from_json(query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

    assert_eq!(Decimal::one(), res);

    let msg = QueryMsg::WithdrawalPercentAllowed {
        timestamp: Some(Milliseconds::zero()),
    };
    let err = query(deps.as_ref(), mock_env(), msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidTimestamp {
            msg: "Provided timestamp is in past".to_string()
        }
    );

    let timestamp = mock_env().block.time.plus_seconds(DEPOSIT_WINDOW + 1);
    let msg = QueryMsg::WithdrawalPercentAllowed {
        timestamp: Some(Milliseconds::from_seconds(timestamp.seconds())),
    };
    let res: Decimal = from_json(query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

    assert_eq!(Decimal::percent(50), res);

    let timestamp = mock_env()
        .block
        .time
        .plus_seconds(DEPOSIT_WINDOW + WITHDRAWAL_WINDOW);
    let msg = QueryMsg::WithdrawalPercentAllowed {
        timestamp: Some(Milliseconds::from_nanos(timestamp.nanos())),
    };
    let res: Decimal = from_json(query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

    assert_eq!(Decimal::zero(), res);
}

// #[test]
// fn test_deposit_to_bootstrap_contract_not_specified() {
//     let mut deps = mock_dependencies_custom(&[]);
//     init(deps.as_mut()).unwrap();

//     let msg = ExecuteMsg::DepositToBootstrap {
//         amount: Uint128::new(100),
//     };
//     let info = mock_info("owner", &[]);

//     let mut env = mock_env();
//     env.block.time = env
//         .block
//         .time
//         .plus_seconds(DEPOSIT_WINDOW + WITHDRAWAL_WINDOW + 1);

//     let res = execute(deps.as_mut(), env, info, msg);

//     assert_eq!(ContractError::NoSavedBootstrapContract {}, res.unwrap_err());
// }
