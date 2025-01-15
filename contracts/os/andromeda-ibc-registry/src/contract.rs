use crate::state::REGISTRY;
use andromeda_std::ado_base::permissioning::{LocalPermission, Permission};
use andromeda_std::common::actions::call_action;
use andromeda_std::os::ibc_registry::{
    verify_denom, AllDenomInfoResponse, DenomInfo, DenomInfoResponse, ExecuteMsg, IBCDenomInfo,
    InstantiateMsg, QueryMsg,
};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    ado_contract::ADOContract,
    common::{context::ExecuteContext, encode_binary},
    error::ContractError,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, ensure, Binary, Deps, DepsMut, Env, MessageInfo, Order, Reply, Response, StdError,
    Storage,
};
use cw_storage_plus::Bound;
use std::collections::HashSet;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-ibc-registry";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const STORE_DENOM_INFO: &str = "StoreDenomInfo";

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
        info,
        BaseInstantiateMsg {
            ado_type: CONTRACT_NAME.to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            kernel_address: msg.kernel_address.into_string(),
            owner: msg.owner,
        },
    )?;

    // Save service address
    let service_address = msg
        .service_address
        .get_raw_address(&deps.as_ref())?
        .into_string();

    ADOContract::default().permission_action(STORE_DENOM_INFO, deps.storage)?;
    ADOContract::set_permission(
        deps.storage,
        STORE_DENOM_INFO,
        service_address.clone(),
        Permission::Local(LocalPermission::whitelisted(None, None)),
    )?;

    Ok(resp.add_attribute("service_address", service_address))
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

fn handle_execute(mut ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    let action = msg.as_ref().to_string();

    let action_response = call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;

    let res = match msg {
        ExecuteMsg::StoreDenomInfo { ibc_denom_info } => {
            execute_store_denom_info(ctx, action, ibc_denom_info)
        }
        _ => ADOContract::default().execute(ctx, msg),
    }?;

    Ok(res
        .add_submessages(action_response.messages)
        .add_attributes(action_response.attributes)
        .add_events(action_response.events))
}

pub fn execute_store_denom_info(
    mut ctx: ExecuteContext,
    action: String,
    ibc_denom_info: Vec<IBCDenomInfo>,
) -> Result<Response, ContractError> {
    let sender = ctx.info.sender.clone();
    // Verify authority
    ADOContract::default().is_permissioned_strict(
        ctx.deps.branch(),
        ctx.env.clone(),
        action,
        sender.clone(),
    )?;

    // Vector can't be empty
    ensure!(
        !ibc_denom_info.is_empty(),
        ContractError::NoDenomInfoProvided {}
    );

    let mut seen_denoms = HashSet::new(); // To track unique denoms
    for info in ibc_denom_info {
        let denom = info.denom;
        verify_denom(&denom, &info.denom_info)?;

        // Check for duplicates
        if !seen_denoms.insert(denom.clone()) {
            return Err(ContractError::DuplicateDenoms { denom });
        }

        // Store the denom info
        REGISTRY.save(ctx.deps.storage, denom, &info.denom_info)?;
    }

    Ok(Response::new().add_attributes(vec![
        attr("action", "store_denom_info"),
        attr("sender", sender),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::DenomInfo { denom } => encode_binary(&get_denom_info(deps.storage, denom)?),
        QueryMsg::AllDenomInfo { limit, start_after } => {
            encode_binary(&get_all_denom_info(deps.storage, limit, start_after)?)
        }
    }
}

pub fn get_denom_info(
    storage: &dyn Storage,
    denom: String,
) -> Result<DenomInfoResponse, ContractError> {
    let denom_info = REGISTRY.load(storage, denom)?;
    Ok(DenomInfoResponse { denom_info })
}

pub fn get_all_denom_info(
    storage: &dyn Storage,
    limit: Option<u64>,
    start_after: Option<u64>,
) -> Result<AllDenomInfoResponse, ContractError> {
    // Convert `start_after` into a Bound if provided
    let min = Some(Bound::inclusive(start_after.unwrap_or(0).to_string()));

    // Set the limit, defaulting to 100 if none is provided
    let limit = limit.unwrap_or(100) as usize;

    // Query the registry with pagination
    let denom_info_iter: Result<Vec<(String, DenomInfo)>, StdError> = REGISTRY
        .range(storage, min, None, Order::Ascending)
        .take(limit)
        .collect();

    // Collect the results into a vector of `DenomInfo`
    let denom_info_list: Vec<DenomInfo> = denom_info_iter?
        .into_iter()
        .map(|(_, denom_info)| denom_info)
        .collect();

    Ok(AllDenomInfoResponse {
        denom_info: denom_info_list,
    })
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
