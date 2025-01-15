use super::mock::{
    proper_initialization, query_current_ado_path, query_cycle_start_time, query_gate_addresses,
    query_time_interval, update_cycle_start_time, update_gate_addresses, update_time_interval,
};
use andromeda_std::error::ContractError;
use andromeda_std::{
    amp::AndrAddr,
    common::{expiration::Expiry, Milliseconds},
};
use cosmwasm_std::{testing::mock_env, BlockInfo, Timestamp};
use cw_utils::Expiration;

#[test]
fn test_instantiation() {
    let (deps, _) = proper_initialization(
        vec![
            AndrAddr::from_string("mock_ado_1".to_string()),
            AndrAddr::from_string("mock_ado_2".to_string()),
            AndrAddr::from_string("mock_ado_3".to_string()),
        ],
        Some(Expiry::FromNow(Milliseconds(5000000000))),
        None,
    );

    let res = query_gate_addresses(deps.as_ref()).unwrap();
    assert_eq!(
        res,
        vec![
            AndrAddr::from_string("mock_ado_1".to_string()),
            AndrAddr::from_string("mock_ado_2".to_string()),
            AndrAddr::from_string("mock_ado_3".to_string()),
        ]
    );

    let res = query_cycle_start_time(deps.as_ref()).unwrap();
    assert_eq!(
        res,
        (
            Expiration::AtTime(Timestamp::from_nanos(5000100000000000)),
            Milliseconds::from_seconds(5000100)
        )
    );

    let res = query_time_interval(deps.as_ref()).unwrap();
    assert_eq!(res, "3600".to_string());
}

#[test]
fn test_update_cycle_start_time() {
    let (mut deps, info) = proper_initialization(
        vec![
            AndrAddr::from_string("mock_ado_1".to_string()),
            AndrAddr::from_string("mock_ado_2".to_string()),
            AndrAddr::from_string("mock_ado_3".to_string()),
        ],
        Some(Expiry::FromNow(Milliseconds(5000000000))),
        None,
    );

    let err_res = update_cycle_start_time(
        deps.as_mut(),
        Some(Expiry::FromNow(Milliseconds(5000000000))),
        info.sender.as_ref(),
    )
    .unwrap_err();

    assert_eq!(
        err_res,
        ContractError::InvalidParameter {
            error: Some("Same as an existed cycle start time".to_string())
        }
    );

    update_cycle_start_time(
        deps.as_mut(),
        Some(Expiry::FromNow(Milliseconds(4000000000))),
        info.sender.as_ref(),
    )
    .unwrap();

    let res = query_cycle_start_time(deps.as_ref()).unwrap();
    assert_eq!(
        res,
        (
            Expiration::AtTime(Timestamp::from_nanos(4000100000000000)),
            Milliseconds::from_seconds(4000100)
        )
    );

    update_time_interval(deps.as_mut(), 7200, info.sender.as_ref()).unwrap();

    let res = query_time_interval(deps.as_ref()).unwrap();
    assert_eq!(res, "7200".to_string(),);
}

#[test]
fn test_update_gate_addresses() {
    let (mut deps, info) = proper_initialization(
        vec![
            AndrAddr::from_string("mock_ado_1".to_string()),
            AndrAddr::from_string("mock_ado_2".to_string()),
            AndrAddr::from_string("mock_ado_3".to_string()),
        ],
        Some(Expiry::FromNow(Milliseconds(5000000000))),
        None,
    );

    let err_res = update_gate_addresses(
        deps.as_mut(),
        vec![
            AndrAddr::from_string("mock_ado_1".to_string()),
            AndrAddr::from_string("mock_ado_2".to_string()),
            AndrAddr::from_string("mock_ado_3".to_string()),
        ],
        info.sender.as_ref(),
    )
    .unwrap_err();

    assert_eq!(
        err_res,
        ContractError::InvalidParameter {
            error: Some("Same as existed gate addresses".to_string())
        }
    );

    update_gate_addresses(
        deps.as_mut(),
        vec![
            AndrAddr::from_string("mock_ado_1".to_string()),
            AndrAddr::from_string("mock_ado_2".to_string()),
            AndrAddr::from_string("mock_ado_3".to_string()),
            AndrAddr::from_string("mock_ado_4".to_string()),
        ],
        info.sender.as_ref(),
    )
    .unwrap();

    let res = query_gate_addresses(deps.as_ref()).unwrap();
    assert_eq!(
        res,
        vec![
            AndrAddr::from_string("mock_ado_1".to_string()),
            AndrAddr::from_string("mock_ado_2".to_string()),
            AndrAddr::from_string("mock_ado_3".to_string()),
            AndrAddr::from_string("mock_ado_4".to_string()),
        ]
    );
}

#[test]
fn test_query_current_ado_path_not_started_yet() {
    let (deps, _) = proper_initialization(
        vec![
            AndrAddr::from_string("mock_ado_1".to_string()),
            AndrAddr::from_string("mock_ado_2".to_string()),
            AndrAddr::from_string("mock_ado_3".to_string()),
        ],
        Some(Expiry::FromNow(Milliseconds(5000000000))),
        None,
    );

    let mut env = mock_env();
    env.block = BlockInfo {
        height: 100,
        time: Timestamp::from_nanos(100000000000u64),
        chain_id: "test-chain".to_string(),
    };

    let res = query_current_ado_path(deps.as_ref(), env).unwrap_err();
    assert_eq!(
        res,
        ContractError::CustomError {
            msg: "Cycle is not started yet".to_string()
        }
    );
}

#[test]
fn test_query_current_ado_path() {
    let (deps, _) = proper_initialization(
        vec![
            AndrAddr::from_string("mock_ado_1".to_string()),
            AndrAddr::from_string("mock_ado_2".to_string()),
            AndrAddr::from_string("mock_ado_3".to_string()),
            AndrAddr::from_string("mock_ado_4".to_string()),
            AndrAddr::from_string("mock_ado_5".to_string()),
        ],
        Some(Expiry::FromNow(Milliseconds(5000000000))),
        None,
    );

    let mut env = mock_env();
    env.block = BlockInfo {
        height: 100,
        time: Timestamp::from_nanos(100000000000u64),
        chain_id: "test-chain".to_string(),
    };

    env.block.time = env.block.time.plus_seconds(5000100);
    let res = query_current_ado_path(deps.as_ref(), env.clone()).unwrap();
    assert_eq!(res, "mock_ado_1".to_string());

    env.block.time = env.block.time.plus_seconds(6000);
    let res = query_current_ado_path(deps.as_ref(), env.clone()).unwrap();
    assert_eq!(res, "mock_ado_2".to_string());

    env.block.time = env.block.time.plus_seconds(3600 * 3);
    let res = query_current_ado_path(deps.as_ref(), env.clone()).unwrap();
    assert_eq!(res, "mock_ado_5".to_string());

    env.block.time = env.block.time.plus_seconds(3600);
    let res = query_current_ado_path(deps.as_ref(), env.clone()).unwrap();
    assert_eq!(res, "mock_ado_1".to_string());
}
