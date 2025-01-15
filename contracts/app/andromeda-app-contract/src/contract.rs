use crate::reply::on_component_instantiation;
use crate::state::{add_app_component, create_cross_chain_message, ADO_ADDRESSES, APP_NAME};
use andromeda_app::app::{ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::ado_contract::ADOContract;
use andromeda_std::amp::AndrAddr;
use andromeda_std::common::context::ExecuteContext;
use andromeda_std::common::reply::ReplyId;
use andromeda_std::os::vfs::{convert_component_name, ExecuteMsg as VFSExecuteMsg};
use andromeda_std::{
    ado_base::InstantiateMsg as BaseInstantiateMsg, ado_base::MigrateMsg, common::encode_binary,
    error::ContractError,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure, wasm_execute, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError,
    SubMsg,
};

use crate::{execute, query};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-app-contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    APP_NAME.save(deps.storage, &msg.name)?;

    ensure!(
        msg.app_components.len() <= 50,
        ContractError::TooManyAppComponents {}
    );

    let sender = msg.owner.clone().unwrap_or(info.sender.to_string());
    let mut resp = ADOContract::default()
        .instantiate(
            deps.storage,
            env.clone(),
            deps.api,
            &deps.querier,
            info.clone(),
            BaseInstantiateMsg {
                ado_type: CONTRACT_NAME.to_string(),
                ado_version: CONTRACT_VERSION.to_string(),
                kernel_address: msg.kernel_address.clone(),
                owner: msg.owner.clone(),
            },
        )?
        .add_attribute("andr_app", msg.name.clone());

    let vfs_address = ADOContract::default().get_vfs_address(deps.storage, &deps.querier)?;
    let adodb_addr = ADOContract::default().get_adodb_address(deps.storage, &deps.querier)?;

    let mut vfs_msgs: Vec<SubMsg> = vec![];

    for component in msg.app_components.clone() {
        ensure!(
            !ADO_ADDRESSES.has(deps.storage, &component.name),
            ContractError::NameAlreadyTaken {}
        );
        component.verify(&deps.as_ref()).unwrap();

        // Generate addresses and store expected address in state
        let new_addr = component.get_new_addr(
            deps.api,
            &adodb_addr,
            &deps.querier,
            env.contract.address.clone(),
        )?;
        ADO_ADDRESSES.save(
            deps.storage,
            &component.name,
            &new_addr.clone().unwrap_or(Addr::unchecked("")),
        )?;

        // Register components with VFS
        // Sub message is optional as component may be hidden (Starts with a '.')
        let register_submsg = component.generate_vfs_registration(
            new_addr.clone(),
            &env.contract.address,
            &msg.name,
            msg.chain_info.clone(),
            &adodb_addr,
            &vfs_address,
        )?;

        if let Some(register_submsg) = register_submsg {
            vfs_msgs.push(register_submsg);
        }

        let event = component.generate_event(new_addr);
        resp = resp.add_event(event);
    }

    let mut inst_msgs = vec![];

    // This is done in a separate loop to ensure ordering, VFS registration first then instantiation after
    for component in msg.app_components.clone() {
        // Generate an ID for the component to help with tracking
        let idx = add_app_component(deps.storage, &component)?;

        // Generate an instantiation message if required
        let inst_msg = component.generate_instantiation_message(
            &deps.querier,
            &adodb_addr,
            &env.contract.address,
            &sender,
            idx,
        )?;

        if let Some(inst_msg) = inst_msg {
            inst_msgs.push(inst_msg)
        }
    }

    // Register app under parent
    let app_name = msg.name;
    let add_path_msg = VFSExecuteMsg::AddChild {
        name: convert_component_name(&app_name),
        parent_address: AndrAddr::from_string(sender),
    };
    let cosmos_msg = wasm_execute(vfs_address.to_string(), &add_path_msg, vec![])?;
    let register_msg = SubMsg::reply_on_error(cosmos_msg, ReplyId::RegisterPath.repr());

    resp = resp
        .add_submessage(register_msg)
        .add_submessages(vfs_msgs)
        .add_submessages(inst_msgs);

    if let Some(chain_info) = msg.chain_info {
        for chain in chain_info.clone() {
            let sub_msg = create_cross_chain_message(
                &deps,
                app_name.clone(),
                msg.owner.clone().unwrap_or(info.sender.to_string()),
                msg.app_components.clone(),
                chain,
                chain_info.clone(),
            )?;
            resp = resp.add_submessage(sub_msg);
        }
    }
    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.result.is_err() {
        return Err(ContractError::Std(StdError::generic_err(
            msg.result.unwrap_err(),
        )));
    }

    match ReplyId::from_repr(msg.id) {
        Some(ReplyId::RegisterPath) => Ok(Response::default()),
        Some(ReplyId::ClaimOwnership) => Ok(Response::default()),
        Some(ReplyId::AssignApp) => Ok(Response::default()),
        _ => on_component_instantiation(deps, msg),
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

pub fn handle_execute(ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AddAppComponent { component } => {
            execute::handle_add_app_component(ctx, component)
        }
        ExecuteMsg::ClaimOwnership { name, new_owner } => {
            execute::claim_ownership(ctx, name, new_owner)
        }
        ExecuteMsg::ProxyMessage { msg, name } => execute::message(ctx, name, msg),
        ExecuteMsg::UpdateAddress { name, addr } => execute::update_address(ctx, name, addr),
        ExecuteMsg::AssignAppToComponents {} => execute::assign_app_to_components(ctx),
        _ => ADOContract::default().execute(ctx, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, env, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetAddress { name } => encode_binary(&query::component_address(deps, name)?),
        QueryMsg::GetAddressesWithNames {} => {
            encode_binary(&query::component_addresses_with_name(deps)?)
        }
        QueryMsg::GetComponents {} => encode_binary(&query::component_descriptors(deps)?),
        QueryMsg::Config {} => encode_binary(&query::config(deps)?),
        QueryMsg::ComponentExists { name } => encode_binary(&query::component_exists(deps, name)),
        _ => ADOContract::default().query(deps, env, msg),
    }
}
