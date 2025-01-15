use andromeda_std::ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg};
use andromeda_std::ado_contract::ADOContract;
use andromeda_std::common::encode_binary;
use andromeda_std::common::reply::ReplyId;
use andromeda_std::error::ContractError;
use andromeda_std::os::economics::{Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg};
#[allow(unused_imports)]
use cosmwasm_std::{
    attr, coin, ensure, entry_point, from_json, to_json_binary, Addr, BankMsg, Binary, CosmosMsg,
    Deps, DepsMut, Empty, Env, MessageInfo, Response, Storage, SubMsg, Uint128, WasmMsg,
};
use cosmwasm_std::{Reply, StdError};
use cw20::Cw20ReceiveMsg;

use crate::{execute, query};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-economics";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        &deps.querier,
        info,
        BaseInstantiateMsg {
            ado_type: CONTRACT_NAME.to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            kernel_address: msg.kernel_address,
            owner: msg.owner,
        },
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.result.is_err() {
        return Err(ContractError::Std(StdError::generic_err(
            msg.result.unwrap_err(),
        )));
    }

    match ReplyId::from_repr(msg.id) {
        Some(ReplyId::Cw20WithdrawMsg) => Err(ContractError::Std(StdError::generic_err(
            msg.result.unwrap_err(),
        ))),
        Some(ReplyId::PayFee) => Err(ContractError::Std(StdError::generic_err(
            msg.result.unwrap_err(),
        ))),
        _ => Ok(Response::default()),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Deposit { address } => execute::deposit_native(deps, info, address),
        ExecuteMsg::PayFee { payee, action } => execute::pay_fee(deps, env, info, payee, action),
        ExecuteMsg::Withdraw { amount, asset } => {
            execute::withdraw_native(deps, info, amount, asset)
        }
        ExecuteMsg::WithdrawCW20 { amount, asset } => {
            execute::withdraw_cw20(deps, info, amount, asset)
        }
        ExecuteMsg::Receive(cw20msg) => cw20_receive(deps, env, info, cw20msg),
        // Base message
        ExecuteMsg::Ownership(ownership_message) => {
            ADOContract::default().execute_ownership(deps, env, info, ownership_message)
        }
    }
}

pub fn cw20_receive(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let sender = deps.api.addr_validate(&msg.sender)?;
    let amount = msg.amount;

    match from_json::<Cw20HookMsg>(&msg.msg)? {
        Cw20HookMsg::Deposit { address } => {
            execute::cw20_deposit(deps, info, sender, amount, address)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, env, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Balance { address, asset } => {
            Ok(to_json_binary(&query::balance(deps, address, asset)?)?)
        }
        // Base queries
        QueryMsg::Version {} => encode_binary(&ADOContract::default().query_version(deps)?),
        QueryMsg::Type {} => encode_binary(&ADOContract::default().query_type(deps)?),
        QueryMsg::Owner {} => encode_binary(&ADOContract::default().query_contract_owner(deps)?),
        QueryMsg::KernelAddress {} => {
            encode_binary(&ADOContract::default().query_kernel_address(deps)?)
        }
    }
}
