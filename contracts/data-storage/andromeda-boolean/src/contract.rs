#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Addr, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError};

use crate::{
    execute::handle_execute,
    query::{get_data_owner, get_value},
    state::RESTRICTION,
};
use andromeda_data_storage::boolean::{BooleanRestriction, ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::{
    ado_base::{
        permissioning::{LocalPermission, Permission},
        InstantiateMsg as BaseInstantiateMsg, MigrateMsg,
    },
    ado_contract::ADOContract,
    common::{context::ExecuteContext, encode_binary},
    error::ContractError,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-boolean";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const SET_DELETE_VALUE_ACTION: &str = "set_delete_value";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let resp = ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        &deps.querier,
        info.clone(),
        BaseInstantiateMsg {
            ado_type: CONTRACT_NAME.to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            kernel_address: msg.kernel_address,
            owner: msg.owner.clone(),
        },
    )?;
    RESTRICTION.save(deps.storage, &msg.restriction)?;

    if msg.restriction == BooleanRestriction::Private {
        ADOContract::default().permission_action(SET_DELETE_VALUE_ACTION, deps.storage)?;

        ADOContract::set_permission(
            deps.storage,
            SET_DELETE_VALUE_ACTION,
            match msg.owner.clone() {
                None => info.sender,
                Some(owner) => Addr::unchecked(owner),
            },
            Permission::Local(LocalPermission::whitelisted(None, None)),
        )?;
    }

    Ok(resp)
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
        ExecuteMsg::AMPReceive(pkt) => {
            ADOContract::default().execute_amp_receive(ctx, pkt, handle_execute)
        }
        _ => handle_execute(ctx, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetValue {} => encode_binary(&get_value(deps.storage)?),
        QueryMsg::GetDataOwner {} => encode_binary(&get_data_owner(deps.storage)?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, env, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.result.is_err() {
        return Err(ContractError::Std(StdError::generic_err(
            msg.result.unwrap_err(),
        )));
    }

    Ok(Response::default())
}
