use crate::{
    contract::{execute, instantiate, query},
    state::{batches, Batch, CONFIG, NEXT_ID},
    testing::mock_querier::mock_dependencies_custom,
};
use andromeda_std::{
    amp::Recipient,
    common::{withdraw::WithdrawalType, Milliseconds},
    error::ContractError,
    testing::mock_querier::MOCK_KERNEL_CONTRACT,
};
use cosmwasm_std::{
    coin, coins, from_json,
    testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR},
    BankMsg, Decimal, DepsMut, Response, Uint128,
};

use andromeda_finance::vesting::{BatchResponse, Config, ExecuteMsg, InstantiateMsg, QueryMsg};

const MOCK_NATIVE_DENOM: &str = "uusd";
fn init(deps: DepsMut) -> Response {
    let msg = InstantiateMsg {
        recipient: Recipient::from_string("recipient"),
        denom: "uusd".to_string(),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let info = mock_info("owner", &[]);
    instantiate(deps, mock_env(), info, msg).unwrap()
}

fn create_batch(
    deps: DepsMut,
    lockup_duration: Option<Milliseconds>,
    release_duration: Milliseconds,
    release_amount: WithdrawalType,
) -> Response {
    // Create batch with half of the release_duration.
    let msg = ExecuteMsg::CreateBatch {
        lockup_duration,
        release_duration,
        release_amount,
    };

    let info = mock_info("owner", &coins(100, "uusd"));
    execute(deps, mock_env(), info, msg).unwrap()
}

#[test]
fn test_instantiate() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    let res = init(deps.as_mut());

    assert_eq!(
        Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("type", "vesting")
            .add_attribute("kernel_address", MOCK_KERNEL_CONTRACT)
            .add_attribute("owner", "owner"),
        res
    );

    assert_eq!(
        Config {
            recipient: Recipient::from_string("recipient"),
            denom: "uusd".to_string(),
        },
        CONFIG.load(deps.as_ref().storage).unwrap()
    );
}

#[test]
fn test_instantiate_invalid_denom() {
    let mut deps = mock_dependencies_custom(&[coin(100000, "uandr")]);

    let msg = InstantiateMsg {
        recipient: Recipient::from_string("recipient"),
        denom: MOCK_NATIVE_DENOM.to_string(),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let info = mock_info("owner", &[]);
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(
        err,
        ContractError::InvalidAsset {
            asset: MOCK_NATIVE_DENOM.to_string()
        }
    )
}

#[test]
fn test_instantiate_invalid_address() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);

    let msg = InstantiateMsg {
        recipient: Recipient::from_string("1"),
        denom: MOCK_NATIVE_DENOM.to_string(),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let info = mock_info("owner", &[]);
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidAddress {})
}
//
#[test]
fn test_create_batch_unauthorized() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());

    let info = mock_info("not_owner", &[]);

    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration: Milliseconds::from_seconds(1),
        release_amount: WithdrawalType::Amount(Uint128::zero()),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
}

#[test]
fn test_create_batch_no_funds() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);

    init(deps.as_mut());

    let info = mock_info("owner", &[]);

    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration: Milliseconds::from_seconds(1),
        release_amount: WithdrawalType::Amount(Uint128::zero()),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        ContractError::InvalidFunds {
            msg: "Creating a batch must be accompanied with a single native fund".to_string()
        },
        res.unwrap_err()
    );
}

#[test]
fn test_create_batch_invalid_denom() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());

    let info = mock_info("owner", &coins(500, "uluna"));

    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration: Milliseconds::from_seconds(1),
        release_amount: WithdrawalType::Amount(Uint128::zero()),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(
        ContractError::InvalidFunds {
            msg: "Invalid denom".to_string()
        },
        res.unwrap_err()
    );
}

#[test]
fn test_create_batch_valid_denom_zero_amount() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());

    let info = mock_info("owner", &coins(0, "uusd"));

    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration: Milliseconds::from_seconds(1),
        release_amount: WithdrawalType::Amount(Uint128::zero()),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(ContractError::InvalidZeroAmount {}, res.unwrap_err());
}

#[test]
fn test_create_batch_release_duration_zero() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());

    let info = mock_info("owner", &coins(100, "uusd"));

    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration: Milliseconds::zero(),
        release_amount: WithdrawalType::Amount(Uint128::zero()),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(ContractError::InvalidZeroAmount {}, res.unwrap_err());
}

#[test]
fn test_create_batch_release_amount_zero() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());

    let info = mock_info("owner", &coins(100, "uusd"));

    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration: Milliseconds::from_seconds(10),
        release_amount: WithdrawalType::Amount(Uint128::zero()),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(ContractError::InvalidZeroAmount {}, res.unwrap_err());
}

#[test]
fn test_create_batch() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());

    let info = mock_info("owner", &coins(100, "uusd"));

    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration: Milliseconds::from_seconds(10),
        release_amount: WithdrawalType::Amount(Uint128::new(10)),
    };

    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    let current_time = Milliseconds::from_seconds(mock_env().block.time.seconds());

    assert_eq!(
        Response::new()
            .add_attribute("action", "create_batch")
            .add_attribute("amount", "100")
            .add_attribute("lockup_end", current_time.to_string())
            .add_attribute(
                "release_duration",
                Milliseconds::from_seconds(10).to_string()
            )
            .add_attribute("release_amount", "Amount(Uint128(10))"),
        res
    );

    let batch = batches().load(deps.as_ref().storage, 1).unwrap();

    assert_eq!(
        Batch {
            amount: Uint128::new(100),
            amount_claimed: Uint128::zero(),
            lockup_end: current_time,
            release_duration: Milliseconds::from_seconds(10),
            release_amount: WithdrawalType::Amount(Uint128::new(10)),
            last_claimed_release_time: current_time,
        },
        batch
    );

    assert_eq!(2, NEXT_ID.load(deps.as_ref().storage).unwrap());

    // Try to create another batch.
    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: Some(Milliseconds::from_seconds(100)),
        release_duration: Milliseconds::from_seconds(10),
        release_amount: WithdrawalType::Amount(Uint128::new(10)),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "create_batch")
            .add_attribute("amount", "100")
            .add_attribute("lockup_end", (current_time.plus_seconds(100)).to_string())
            .add_attribute(
                "release_duration",
                Milliseconds::from_seconds(10).to_string()
            )
            .add_attribute("release_amount", "Amount(Uint128(10))"),
        res
    );

    let batch = batches().load(deps.as_ref().storage, 2).unwrap();

    assert_eq!(
        Batch {
            amount: Uint128::new(100),
            amount_claimed: Uint128::zero(),
            lockup_end: current_time.plus_seconds(100),
            release_duration: Milliseconds::from_seconds(10),
            release_amount: WithdrawalType::Amount(Uint128::new(10)),
            last_claimed_release_time: current_time.plus_seconds(100),
        },
        batch
    );

    assert_eq!(3, NEXT_ID.load(deps.as_ref().storage).unwrap());
}

#[test]
fn test_claim_batch_unauthorized() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());

    let info = mock_info("not_owner", &[]);

    let msg = ExecuteMsg::Claim {
        number_of_claims: None,
        batch_id: 1,
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
}

#[test]
fn test_claim_batch_still_locked() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());
    let info = mock_info("owner", &coins(100, "uusd"));

    // Create batch.
    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: Some(Milliseconds::from_seconds(100)),
        release_duration: Milliseconds::from_seconds(10),
        release_amount: WithdrawalType::Amount(Uint128::new(10)),
    };

    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // Claim batch.
    let msg = ExecuteMsg::Claim {
        number_of_claims: None,
        batch_id: 1,
    };

    let res = execute(deps.as_mut(), mock_env(), mock_info("owner", &[]), msg);

    assert_eq!(ContractError::FundsAreLocked {}, res.unwrap_err());
}

#[test]
fn test_claim_batch_no_funds_available() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());
    let info = mock_info("owner", &coins(100, "uusd"));

    // Create batch.
    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration: Milliseconds::from_seconds(10),
        release_amount: WithdrawalType::Amount(Uint128::new(10)),
    };

    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // Claim batch.
    let msg = ExecuteMsg::Claim {
        number_of_claims: None,
        batch_id: 1,
    };

    let res = execute(deps.as_mut(), mock_env(), mock_info("owner", &[]), msg);

    // This is because, the first payment becomes available after 10 seconds.
    assert_eq!(ContractError::WithdrawalIsEmpty {}, res.unwrap_err());
}

#[test]
fn test_claim_batch_single_claim() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());
    let info = mock_info("owner", &coins(100, "uusd"));

    let release_duration = Milliseconds::from_seconds(10);

    // Create batch.
    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration,
        release_amount: WithdrawalType::Amount(Uint128::new(10)),
    };

    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    deps.querier
        .base
        .update_balance(MOCK_CONTRACT_ADDR, coins(100, "uusd"));

    // Skip time.
    let mut env = mock_env();
    // A single release is available.
    env.block.time = env.block.time.plus_seconds(release_duration.seconds());

    // Query created batch.
    let msg = QueryMsg::Batch { id: 1 };
    let res: BatchResponse = from_json(query(deps.as_ref(), env.clone(), msg).unwrap()).unwrap();

    let lockup_end = Milliseconds::from_seconds(mock_env().block.time.seconds());
    assert_eq!(
        BatchResponse {
            id: 1,
            amount: Uint128::new(100),
            amount_claimed: Uint128::zero(),
            amount_available_to_claim: Uint128::new(10),
            number_of_available_claims: Uint128::new(1),
            lockup_end,
            release_duration,
            release_amount: WithdrawalType::Amount(Uint128::new(10)),
            last_claimed_release_time: lockup_end,
        },
        res
    );

    // Claim batch.
    let msg = ExecuteMsg::Claim {
        number_of_claims: None,
        batch_id: 1,
    };

    let res = execute(deps.as_mut(), env, mock_info("owner", &[]), msg).unwrap();

    assert_eq!(
        Response::new()
            .add_message(BankMsg::Send {
                to_address: "recipient".to_string(),
                amount: coins(10, "uusd")
            })
            .add_attribute("action", "claim")
            .add_attribute("amount", "10")
            .add_attribute("batch_id", "1")
            .add_attribute("amount_left", "90"),
        res
    );
    let lockup_end = Milliseconds::from_seconds(mock_env().block.time.seconds());

    assert_eq!(
        Batch {
            amount: Uint128::new(100),
            amount_claimed: Uint128::new(10),
            lockup_end,
            release_duration: Milliseconds::from_seconds(10),
            release_amount: WithdrawalType::Amount(Uint128::new(10)),
            last_claimed_release_time: lockup_end.plus_milliseconds(release_duration),
        },
        batches().load(deps.as_ref().storage, 1u64).unwrap()
    );
}

#[test]
fn test_claim_batch_not_nice_numbers_single_release() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());
    let info = mock_info("owner", &coins(10, "uusd"));

    let release_duration = Milliseconds::from_seconds(10);

    // Create batch.
    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration,
        release_amount: WithdrawalType::Amount(Uint128::new(10)),
    };

    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    deps.querier
        .base
        .update_balance(MOCK_CONTRACT_ADDR, coins(7, "uusd"));

    // Skip time.
    let mut env = mock_env();
    // A single release is available.
    env.block.time = env.block.time.plus_seconds(release_duration.seconds());

    // Claim batch.
    let msg = ExecuteMsg::Claim {
        number_of_claims: None,
        batch_id: 1,
    };

    let res = execute(deps.as_mut(), env, mock_info("owner", &[]), msg).unwrap();

    assert_eq!(
        Response::new()
            .add_message(BankMsg::Send {
                to_address: "recipient".to_string(),
                amount: coins(7, "uusd")
            })
            .add_attribute("action", "claim")
            .add_attribute("amount", "7")
            .add_attribute("batch_id", "1")
            .add_attribute("amount_left", "3"),
        res
    );
    let lockup_end = Milliseconds::from_seconds(mock_env().block.time.seconds());

    assert_eq!(
        Batch {
            amount: Uint128::new(10),
            amount_claimed: Uint128::new(7),
            lockup_end,
            release_duration: Milliseconds::from_seconds(10),
            release_amount: WithdrawalType::Amount(Uint128::new(10)),
            last_claimed_release_time: lockup_end.plus_milliseconds(release_duration),
        },
        batches().load(deps.as_ref().storage, 1u64).unwrap()
    );
}

#[test]
fn test_claim_batch_not_nice_numbers_multiple_releases() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());
    let vesting_amount = 1_000_000_000_000_000_000u128;
    let info = mock_info("owner", &coins(vesting_amount, "uusd"));

    let release_duration = Milliseconds::from_seconds(1); // 1 second
    let duration: u64 = 60 * 60 * 24 * 365 * 5; // 5 years
    let percent_release = Decimal::from_ratio(Uint128::one(), Uint128::from(duration));

    // Create batch.
    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration,
        release_amount: WithdrawalType::Percentage(percent_release),
    };

    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    deps.querier
        .base
        .update_balance(MOCK_CONTRACT_ADDR, coins(vesting_amount, "uusd"));

    // Skip time.
    let mut env = mock_env();
    // Two releases are out.
    env.block.time = env.block.time.plus_seconds(2 * release_duration.seconds());

    // Claim batch.
    let msg = ExecuteMsg::Claim {
        number_of_claims: None,
        batch_id: 1,
    };

    let res = execute(
        deps.as_mut(),
        env.clone(),
        mock_info("owner", &[]),
        msg.clone(),
    )
    .unwrap();

    assert_eq!(
        Response::new()
            .add_message(BankMsg::Send {
                to_address: "recipient".to_string(),
                amount: coins(12683916792, "uusd")
            })
            .add_attribute("action", "claim")
            .add_attribute("amount", "12683916792")
            .add_attribute("batch_id", "1")
            .add_attribute("amount_left", (vesting_amount - 12683916792).to_string()),
        res
    );
    let lockup_end = Milliseconds::from_seconds(mock_env().block.time.seconds());

    assert_eq!(
        Batch {
            amount: Uint128::new(vesting_amount),
            amount_claimed: Uint128::new(12683916792),
            lockup_end,
            release_duration: Milliseconds::from_seconds(1),
            release_amount: WithdrawalType::Percentage(percent_release),
            last_claimed_release_time: lockup_end.plus_seconds(2 * release_duration.seconds()),
        },
        batches().load(deps.as_ref().storage, 1u64).unwrap()
    );

    env.block.time = env.block.time.plus_seconds(duration);

    let res = execute(deps.as_mut(), env, mock_info("owner", &[]), msg).unwrap();
    assert_eq!(
        Response::new()
            .add_message(BankMsg::Send {
                to_address: "recipient".to_string(),
                amount: coins(vesting_amount - 12683916792, "uusd")
            })
            .add_attribute("action", "claim")
            .add_attribute("amount", (vesting_amount - 12683916792).to_string())
            .add_attribute("batch_id", "1")
            .add_attribute("amount_left", "0"),
        res
    );

    assert_eq!(
        Batch {
            amount: Uint128::new(vesting_amount),
            amount_claimed: Uint128::from(vesting_amount),
            lockup_end,
            release_duration: Milliseconds::from_seconds(1),
            release_amount: WithdrawalType::Percentage(percent_release),
            last_claimed_release_time: lockup_end.plus_seconds(duration + 2),
        },
        batches().load(deps.as_ref().storage, 1u64).unwrap()
    );
}

#[test]
fn test_claim_batch_middle_of_interval() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());
    let info = mock_info("owner", &coins(100, "uusd"));

    let release_duration = Milliseconds::from_seconds(10);

    // Create batch.
    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration,
        release_amount: WithdrawalType::Amount(Uint128::new(10)),
    };

    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    deps.querier
        .base
        .update_balance(MOCK_CONTRACT_ADDR, coins(100, "uusd"));

    // Claim batch.
    let msg = ExecuteMsg::Claim {
        number_of_claims: None,
        batch_id: 1,
    };

    let mut env = mock_env();
    // Only halfway to first release.
    env.block.time = env.block.time.plus_seconds(release_duration.seconds() / 2);

    let res = execute(
        deps.as_mut(),
        env.clone(),
        mock_info("owner", &[]),
        msg.clone(),
    );

    assert_eq!(ContractError::WithdrawalIsEmpty {}, res.unwrap_err());

    // First release available and halfway to second -> result is rounding down.
    env.block.time = env.block.time.plus_seconds(release_duration.seconds());
    let res = execute(deps.as_mut(), env, mock_info("owner", &[]), msg).unwrap();

    assert_eq!(
        Response::new()
            .add_message(BankMsg::Send {
                to_address: "recipient".to_string(),
                amount: coins(10, "uusd")
            })
            .add_attribute("action", "claim")
            .add_attribute("amount", "10")
            .add_attribute("batch_id", "1")
            .add_attribute("amount_left", "90"),
        res
    );
    let lockup_end = Milliseconds::from_seconds(mock_env().block.time.seconds());

    assert_eq!(
        Batch {
            amount: Uint128::new(100),
            amount_claimed: Uint128::new(10),
            lockup_end,
            release_duration: Milliseconds::from_seconds(10),
            release_amount: WithdrawalType::Amount(Uint128::new(10)),
            last_claimed_release_time: lockup_end.plus_milliseconds(release_duration),
        },
        batches().load(deps.as_ref().storage, 1u64).unwrap()
    );
}

#[test]
fn test_claim_batch_multiple_claims() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());
    let info = mock_info("owner", &coins(100, "uusd"));

    let release_duration = Milliseconds::from_seconds(10);

    // Create batch.
    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration,
        release_amount: WithdrawalType::Amount(Uint128::new(10)),
    };

    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    deps.querier
        .base
        .update_balance(MOCK_CONTRACT_ADDR, coins(100, "uusd"));

    let mut env = mock_env();

    // 4 releases are available.
    env.block.time = env.block.time.plus_seconds(4 * release_duration.seconds());

    // Claim only the first release.
    let msg = ExecuteMsg::Claim {
        number_of_claims: Some(1),
        batch_id: 1,
    };
    let res = execute(deps.as_mut(), env.clone(), mock_info("owner", &[]), msg).unwrap();

    assert_eq!(
        Response::new()
            .add_message(BankMsg::Send {
                to_address: "recipient".to_string(),
                amount: coins(10, "uusd")
            })
            .add_attribute("action", "claim")
            .add_attribute("amount", "10")
            .add_attribute("batch_id", "1")
            .add_attribute("amount_left", "90"),
        res
    );
    let lockup_end = Milliseconds::from_seconds(mock_env().block.time.seconds());

    assert_eq!(
        Batch {
            amount: Uint128::new(100),
            amount_claimed: Uint128::new(10),
            lockup_end,
            release_duration,
            release_amount: WithdrawalType::Amount(Uint128::new(10)),
            last_claimed_release_time: lockup_end.plus_milliseconds(release_duration),
        },
        batches().load(deps.as_ref().storage, 1u64).unwrap()
    );

    // Claim the rest of the releases.
    let msg = ExecuteMsg::Claim {
        number_of_claims: None,
        batch_id: 1,
    };
    let res = execute(deps.as_mut(), env, mock_info("owner", &[]), msg).unwrap();

    assert_eq!(
        Response::new()
            .add_message(BankMsg::Send {
                to_address: "recipient".to_string(),
                amount: coins(30, "uusd")
            })
            .add_attribute("action", "claim")
            .add_attribute("amount", "30")
            .add_attribute("batch_id", "1")
            .add_attribute("amount_left", "60"),
        res
    );
    let lockup_end = Milliseconds::from_seconds(mock_env().block.time.seconds());

    assert_eq!(
        Batch {
            amount: Uint128::new(100),
            amount_claimed: Uint128::new(40),
            lockup_end,
            release_duration,
            release_amount: WithdrawalType::Amount(Uint128::new(10)),
            last_claimed_release_time: lockup_end.plus_seconds(4 * release_duration.seconds()),
        },
        batches().load(deps.as_ref().storage, 1u64).unwrap()
    );
}

#[test]
fn test_claim_batch_all_releases() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());
    let info = mock_info("owner", &coins(100, "uusd"));

    let release_duration = Milliseconds::from_seconds(10);

    // Create batch.
    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration,
        release_amount: WithdrawalType::Amount(Uint128::new(10)),
    };

    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    deps.querier
        .base
        .update_balance(MOCK_CONTRACT_ADDR, coins(100, "uusd"));

    let mut env = mock_env();

    // All releases are available and then some (10 * release_duration would be when all releases
    // become available).
    env.block.time = env.block.time.plus_seconds(15 * release_duration.seconds());

    // Claim only the first release.
    let msg = ExecuteMsg::Claim {
        number_of_claims: None,
        batch_id: 1,
    };
    let res = execute(
        deps.as_mut(),
        env.clone(),
        mock_info("owner", &[]),
        msg.clone(),
    )
    .unwrap();

    assert_eq!(
        Response::new()
            .add_message(BankMsg::Send {
                to_address: "recipient".to_string(),
                amount: coins(100, "uusd")
            })
            .add_attribute("action", "claim")
            .add_attribute("amount", "100")
            .add_attribute("batch_id", "1")
            .add_attribute("amount_left", "0"),
        res
    );
    let lockup_end = Milliseconds::from_seconds(mock_env().block.time.seconds());

    assert_eq!(
        Batch {
            amount: Uint128::new(100),
            amount_claimed: Uint128::new(100),
            lockup_end,
            release_duration,
            release_amount: WithdrawalType::Amount(Uint128::new(10)),
            last_claimed_release_time: lockup_end.plus_seconds(15 * release_duration.seconds()),
        },
        batches().load(deps.as_ref().storage, 1u64).unwrap()
    );

    // Try to claim again.
    let res = execute(deps.as_mut(), env, mock_info("owner", &[]), msg);

    assert_eq!(ContractError::WithdrawalIsEmpty {}, res.unwrap_err());
}

#[test]
fn test_claim_batch_too_high_of_claim() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());
    let info = mock_info("owner", &coins(100, "uusd"));

    let release_duration = Milliseconds::from_seconds(10);

    // Create batch.
    let msg = ExecuteMsg::CreateBatch {
        lockup_duration: None,
        release_duration,
        release_amount: WithdrawalType::Amount(Uint128::new(10)),
    };

    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    deps.querier
        .base
        .update_balance(MOCK_CONTRACT_ADDR, coins(100, "uusd"));

    let mut env = mock_env();
    // A single release is available.
    env.block.time = env.block.time.plus_seconds(release_duration.seconds());

    // Try to claim 3 releases.
    let msg = ExecuteMsg::Claim {
        number_of_claims: Some(3),
        batch_id: 1,
    };

    let res = execute(deps.as_mut(), env, mock_info("owner", &[]), msg).unwrap();

    assert_eq!(
        Response::new()
            .add_message(BankMsg::Send {
                to_address: "recipient".to_string(),
                // Only one gets claim
                amount: coins(10, "uusd")
            })
            .add_attribute("action", "claim")
            .add_attribute("amount", "10")
            .add_attribute("batch_id", "1")
            .add_attribute("amount_left", "90"),
        res
    );
    let lockup_end = Milliseconds::from_seconds(mock_env().block.time.seconds());

    assert_eq!(
        Batch {
            amount: Uint128::new(100),
            amount_claimed: Uint128::new(10),
            lockup_end,
            release_duration: Milliseconds::from_seconds(10),
            release_amount: WithdrawalType::Amount(Uint128::new(10)),
            last_claimed_release_time: lockup_end.plus_milliseconds(release_duration),
        },
        batches().load(deps.as_ref().storage, 1u64).unwrap()
    );
}

#[test]
fn test_claim_all_unauthorized() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());

    let info = mock_info("not_owner", &[]);

    let msg = ExecuteMsg::ClaimAll {
        up_to_time: None,
        limit: None,
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
}

#[test]
fn test_claim_all() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    init(deps.as_mut());

    let release_duration = Milliseconds::from_seconds(10);

    let release_amount = WithdrawalType::Amount(Uint128::new(10));
    // Create batch.
    create_batch(
        deps.as_mut(),
        None,
        release_duration,
        release_amount.clone(),
    );

    // Create batch with half of the release_duration.
    create_batch(
        deps.as_mut(),
        None,
        Milliseconds::from_seconds(release_duration.seconds() / 2),
        release_amount.clone(),
    );

    // Create batch with a different release_duration scale (not a factor).
    create_batch(
        deps.as_mut(),
        None,
        Milliseconds::from_seconds(12),
        release_amount.clone(),
    );

    // Create batch that is still locked up.
    create_batch(
        deps.as_mut(),
        Some(Milliseconds::from_seconds(100)),
        release_duration,
        release_amount.clone(),
    );

    deps.querier
        .base
        .update_balance(MOCK_CONTRACT_ADDR, coins(400, "uusd"));

    // Speed up time.
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(release_duration.seconds() * 2);

    // Query batches
    let msg = QueryMsg::Batches {
        start_after: None,
        limit: None,
    };
    let res: Vec<BatchResponse> =
        from_json(query(deps.as_ref(), env.clone(), msg).unwrap()).unwrap();

    let lockup_end = Milliseconds::from_seconds(mock_env().block.time.seconds());
    assert_eq!(
        vec![
            BatchResponse {
                id: 1,
                amount: Uint128::new(100),
                amount_claimed: Uint128::zero(),
                amount_available_to_claim: Uint128::new(20),
                number_of_available_claims: Uint128::new(2),
                lockup_end,
                release_duration,
                release_amount: WithdrawalType::Amount(Uint128::new(10)),
                last_claimed_release_time: lockup_end,
            },
            BatchResponse {
                id: 2,
                amount: Uint128::new(100),
                amount_claimed: Uint128::zero(),
                amount_available_to_claim: Uint128::new(40),
                number_of_available_claims: Uint128::new(4),
                lockup_end,
                release_duration: Milliseconds::from_seconds(release_duration.seconds() / 2),
                release_amount: WithdrawalType::Amount(Uint128::new(10)),
                last_claimed_release_time: lockup_end,
            },
            BatchResponse {
                id: 3,
                amount: Uint128::new(100),
                amount_claimed: Uint128::zero(),
                amount_available_to_claim: Uint128::new(10),
                number_of_available_claims: Uint128::new(1),
                lockup_end,
                release_duration: Milliseconds::from_seconds(12),
                release_amount: WithdrawalType::Amount(Uint128::new(10)),
                last_claimed_release_time: lockup_end,
            },
            BatchResponse {
                id: 4,
                amount: Uint128::new(100),
                amount_claimed: Uint128::zero(),
                amount_available_to_claim: Uint128::zero(),
                number_of_available_claims: Uint128::zero(),
                lockup_end: lockup_end.plus_seconds(100),
                release_duration,
                release_amount: WithdrawalType::Amount(Uint128::new(10)),
                last_claimed_release_time: lockup_end.plus_seconds(100),
            },
        ],
        res
    );

    // Claim all
    let msg = ExecuteMsg::ClaimAll {
        up_to_time: None,
        limit: None,
    };

    let info = mock_info("owner", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_message(BankMsg::Send {
                to_address: "recipient".to_string(),
                // 20 from the first, 40 from the second, 10 from the third.
                amount: coins(20 + 40 + 10, "uusd")
            })
            .add_attribute("action", "claim_all")
            .add_attribute("last_batch_id_processed", "3"),
        res
    );

    let lockup_end = Milliseconds::from_seconds(mock_env().block.time.seconds());
    assert_eq!(
        Batch {
            amount: Uint128::new(100),
            amount_claimed: Uint128::new(20),
            lockup_end,
            release_duration,
            release_amount: release_amount.clone(),
            last_claimed_release_time: lockup_end.plus_seconds(release_duration.seconds() * 2),
        },
        batches().load(deps.as_ref().storage, 1u64).unwrap()
    );

    assert_eq!(
        Batch {
            amount: Uint128::new(100),
            amount_claimed: Uint128::new(40),
            lockup_end,
            release_duration: Milliseconds::from_seconds(release_duration.seconds() / 2),
            release_amount: release_amount.clone(),
            last_claimed_release_time: lockup_end.plus_seconds(release_duration.seconds() * 2),
        },
        batches().load(deps.as_ref().storage, 2u64).unwrap()
    );

    assert_eq!(
        Batch {
            amount: Uint128::new(100),
            amount_claimed: Uint128::new(10),
            lockup_end,
            release_duration: Milliseconds::from_seconds(12),
            release_amount,
            last_claimed_release_time: lockup_end.plus_seconds(12),
        },
        batches().load(deps.as_ref().storage, 3u64).unwrap()
    );
}
