use crate::state::DEFAULT_VALIDATOR;
use cosmwasm_std::{
    ensure, entry_point, Addr, Binary, Deps, DepsMut, Env, FullDelegation, MessageInfo, Response,
    StakingMsg,
};
use cw2::set_contract_version;

use andromeda_finance::validator_staking::{is_validator, ExecuteMsg, InstantiateMsg, QueryMsg};

use andromeda_std::{
    ado_base::InstantiateMsg as BaseInstantiateMsg,
    ado_contract::ADOContract,
    common::{context::ExecuteContext, encode_binary},
    error::ContractError,
};

const CONTRACT_NAME: &str = "crates.io:andromeda-validator-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    msg.validate(&deps)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    DEFAULT_VALIDATOR.save(deps.storage, &msg.default_validator)?;

    let inst_resp = ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        info,
        BaseInstantiateMsg {
            ado_type: "validator-staking".to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            operators: None,
            kernel_address: msg.kernel_address,
            owner: msg.owner,
        },
    )?;
    Ok(inst_resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let ctx = ExecuteContext::new(deps, info, env);

    match msg {
        ExecuteMsg::Stake { validator } => execute_stake(ctx, validator),
        _ => ADOContract::default().execute(ctx, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::StakedTokens { validator } => {
            encode_binary(&query_staked_tokens(deps, env.contract.address, validator)?)
        }
        _ => ADOContract::default().query(deps, env, msg),
    }
}

fn execute_stake(ctx: ExecuteContext, validator: Option<Addr>) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;

    // Ensure only one type of coin is received
    ensure!(
        info.funds.len() == 1,
        ContractError::ExceedsMaxAllowedCoins {}
    );

    let default_validator = DEFAULT_VALIDATOR.load(deps.storage)?;

    // Use default validator if validator is not specified by stake msg
    let validator = validator.unwrap_or(default_validator);

    // Check if the validator is valid before staking
    is_validator(&deps, &validator)?;

    // Delegate funds to the validator

    let funds = &info.funds[0];

    let res = Response::new()
        .add_message(StakingMsg::Delegate {
            validator: validator.to_string(),
            amount: funds.clone(),
        })
        .add_attribute("action", "validator-stake")
        .add_attribute("from", info.sender)
        .add_attribute("to", validator.to_string())
        .add_attribute("amount", funds.amount);

    Ok(res)
}

fn query_staked_tokens(
    deps: Deps,
    delegator: Addr,
    validator: Option<Addr>,
) -> Result<FullDelegation, ContractError> {
    let default_validator = DEFAULT_VALIDATOR.load(deps.storage)?;

    // Use default validator if validator is not specified
    let validator = validator.unwrap_or(default_validator);

    let Some(res) = deps.querier.query_delegation(delegator.to_string(), validator.to_string())? else {
        return Err(ContractError::InvalidDelegation {});
    };
    Ok(res)
}
