use andromeda_std::ado_contract::ADOContract;
use andromeda_std::common::context::ExecuteContext;
use andromeda_std::common::reply::ReplyId;
use andromeda_std::os::vfs::{convert_component_name, ExecuteMsg as VFSExecuteMsg};
use andromeda_std::{
    ado_base::InstantiateMsg as BaseInstantiateMsg, ado_base::MigrateMsg, common::encode_binary,
    error::ContractError,
};
use andromeda_systems::current_block::{ExecuteMsg, InstantiateMsg, QueryMsg};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    wasm_execute, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError, SubMsg,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-system-current-block";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let mut resp = ADOContract::default().instantiate(
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
    )?;

    let vfs_address = ADOContract::default().get_vfs_address(deps.storage, &deps.querier)?;

    let system_ado_name = msg.name;
    let root_directory = msg.root;
    let add_path_msg = VFSExecuteMsg::AddSystemAdoPath {
        name: convert_component_name(&system_ado_name),
        root: convert_component_name(&root_directory),
    };
    let cosmos_msg = wasm_execute(vfs_address.to_string(), &add_path_msg, vec![])?;
    let register_msg = SubMsg::reply_on_error(cosmos_msg, ReplyId::RegisterPath.repr());

    resp = resp.add_submessage(register_msg);

    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.result.is_err() {
        return Err(ContractError::Std(StdError::generic_err(
            msg.result.unwrap_err(),
        )));
    }

    match ReplyId::from_repr(msg.id) {
        Some(ReplyId::RegisterPath) => Ok(Response::default()),
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
    let ctx = ExecuteContext::new(deps, info, env);
    match msg {
        ExecuteMsg::AMPReceive(pkt) => {
            ADOContract::default().execute_amp_receive(ctx, pkt, handle_execute)
        }
        _ => handle_execute(ctx, msg),
    }
}

#[allow(clippy::match_single_binding)]
pub fn handle_execute(ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    match msg {
        _ => ADOContract::default().execute(ctx, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetCurrentBlockHeight {} => encode_binary(&get_current_block_height(env)?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

pub fn get_current_block_height(env: Env) -> Result<String, ContractError> {
    let current_block_height = env.block.height;
    Ok(current_block_height.to_string())
}
