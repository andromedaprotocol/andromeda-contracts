use andromeda_std::ado_contract::ADOContract;
use andromeda_std::os::vfs::{ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    common::encode_binary,
    error::ContractError,
};
use cosmwasm_std::{
    entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError,
};

use crate::{execute, query};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-vfs";
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

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let execute_env = execute::ExecuteEnv { deps, env, info };

    match msg {
        ExecuteMsg::AddPath {
            name,
            address,
            parent_address,
        } => execute::add_path(execute_env, name, address, parent_address),
        ExecuteMsg::AddSymlink {
            name,
            symlink,
            parent_address,
        } => execute::add_symlink(execute_env, name, symlink, parent_address),
        ExecuteMsg::RegisterUser { username, address } => {
            execute::register_user(execute_env, username, address)
        }
        ExecuteMsg::AddChild {
            name,
            parent_address,
        } => execute::add_child(execute_env, name, parent_address),
        ExecuteMsg::AddSystemAdoPath { name, root } => {
            execute::add_system_ado_path(execute_env, name, root)
        }
        ExecuteMsg::RegisterLibrary {
            lib_name,
            lib_address,
        } => execute::register_library(execute_env, lib_name, lib_address),
        ExecuteMsg::RegisterUserCrossChain { chain, address } => {
            execute::register_user_cross_chain(execute_env, chain, address)
        }
        // Base message
        ExecuteMsg::Ownership(ownership_message) => ADOContract::default().execute_ownership(
            execute_env.deps,
            execute_env.env,
            execute_env.info,
            ownership_message,
        ),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::ResolvePath { path } => encode_binary(&query::resolve_path(deps, path)?),
        QueryMsg::SubDir {
            path,
            min,
            max,
            limit,
        } => encode_binary(&query::subdir(deps, path, min, max, limit)?),
        QueryMsg::SubSystem {
            root,
            min,
            max,
            limit,
        } => encode_binary(&query::subsystem(deps, root, min, max, limit)?),
        QueryMsg::Paths { addr } => encode_binary(&query::paths(deps, addr)?),
        QueryMsg::SystemPaths { addr } => encode_binary(&query::system_paths(deps, addr)?),
        QueryMsg::GetUsername { address } => encode_binary(&query::get_username(deps, address)?),
        QueryMsg::GetLibrary { address } => encode_binary(&query::get_library_name(deps, address)?),
        QueryMsg::ResolveSymlink { path } => encode_binary(&query::get_symlink(deps, path)?),
        // Base queries
        QueryMsg::Version {} => encode_binary(&ADOContract::default().query_version(deps)?),
        QueryMsg::Type {} => encode_binary(&ADOContract::default().query_type(deps)?),
        QueryMsg::Owner {} => encode_binary(&ADOContract::default().query_contract_owner(deps)?),
        QueryMsg::KernelAddress {} => {
            encode_binary(&ADOContract::default().query_kernel_address(deps)?)
        }
    }
}
