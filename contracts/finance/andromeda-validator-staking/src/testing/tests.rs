use crate::{
    contract::{execute, instantiate},
    testing::mock_querier::{mock_dependencies_custom, DEFAULT_VALIDATOR, VALID_VALIDATOR},
};

use andromeda_std::{error::ContractError, testing::mock_querier::MOCK_KERNEL_CONTRACT};
use cosmwasm_std::{
    coin,
    testing::{mock_env, mock_info},
    Addr, DepsMut, Response, StakingMsg,
};

use andromeda_finance::validator_staking::{ExecuteMsg, InstantiateMsg};

fn init(deps: DepsMut, default_validator: Addr) -> Result<Response, ContractError> {
    let contract_address = mock_env().contract.address.to_string();

    let msg = InstantiateMsg {
        default_validator,
        owner: Some(contract_address.clone()),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
    };

    let info = mock_info(&contract_address, &[]);
    instantiate(deps, mock_env(), info, msg)
}

#[test]
fn test_instantiate() {
    let mut deps = mock_dependencies_custom();

    let fake_validator = Addr::unchecked("fake_validator");
    let res = init(deps.as_mut(), fake_validator);
    assert_eq!(ContractError::InvalidValidator {}, res.unwrap_err());

    let default_validator = Addr::unchecked(DEFAULT_VALIDATOR);
    let res = init(deps.as_mut(), default_validator).unwrap();
    assert_eq!(0, res.messages.len());
}

#[test]
fn test_stake_with_invalid_funds() {
    let mut deps = mock_dependencies_custom();
    let default_validator = Addr::unchecked(DEFAULT_VALIDATOR);
    init(deps.as_mut(), default_validator).unwrap();

    let msg = ExecuteMsg::Stake { validator: None };
    let contract_address = mock_env().contract.address.to_string();

    let info = mock_info(&contract_address, &[coin(100, "uandr"), coin(100, "usdc")]);

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    assert_eq!(res, ContractError::ExceedsMaxAllowedCoins {});
}

#[test]
fn test_stake_with_default_validator() {
    let mut deps = mock_dependencies_custom();
    let default_validator = Addr::unchecked(DEFAULT_VALIDATOR);
    init(deps.as_mut(), default_validator).unwrap();

    let msg = ExecuteMsg::Stake { validator: None };
    let contract_address = mock_env().contract.address.to_string();

    let info = mock_info(&contract_address, &[coin(100, "uandr")]);

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    let expected_res: Response = Response::new()
        .add_message(StakingMsg::Delegate {
            validator: DEFAULT_VALIDATOR.to_string(),
            amount: coin(100, "uandr"),
        })
        .add_attribute("action", "validator-stake")
        .add_attribute("from", contract_address)
        .add_attribute("to", DEFAULT_VALIDATOR.to_string())
        .add_attribute("amount", "100".to_string());

    assert_eq!(res.unwrap(), expected_res);
}

#[test]
fn test_stake_with_validator() {
    let mut deps = mock_dependencies_custom();
    let default_validator = Addr::unchecked(DEFAULT_VALIDATOR);
    let valid_validator = Addr::unchecked(VALID_VALIDATOR);
    init(deps.as_mut(), default_validator).unwrap();

    let msg = ExecuteMsg::Stake {
        validator: Some(valid_validator),
    };

    let contract_address = mock_env().contract.address.to_string();
    let info = mock_info(&contract_address, &[coin(100, "uandr")]);

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    let expected_res: Response = Response::new()
        .add_message(StakingMsg::Delegate {
            validator: VALID_VALIDATOR.to_string(),
            amount: coin(100, "uandr"),
        })
        .add_attribute("action", "validator-stake")
        .add_attribute("from", contract_address)
        .add_attribute("to", VALID_VALIDATOR.to_string())
        .add_attribute("amount", "100".to_string());

    assert_eq!(res.unwrap(), expected_res);
}

#[test]
fn test_stake_with_invalid_validator() {
    let mut deps = mock_dependencies_custom();
    let fake_validator = Addr::unchecked("fake_validator");
    let default_validator = Addr::unchecked(DEFAULT_VALIDATOR);
    init(deps.as_mut(), default_validator).unwrap();

    let msg = ExecuteMsg::Stake {
        validator: Some(fake_validator),
    };

    let contract_address = mock_env().contract.address.to_string();
    let info = mock_info(&contract_address, &[coin(100, "uandr")]);

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    assert_eq!(res, ContractError::InvalidValidator {});
}
