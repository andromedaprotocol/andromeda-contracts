#[cfg(not(feature = "library"))]
use crate::state::{CURVE_CONFIG, DEFAULT_CONSTANT_VALUE, DEFAULT_MULTIPLE_VARIABLE_VALUE};
use andromeda_math::curve::{
    CurveConfig, CurveType, ExecuteMsg, GetCurveConfigResponse, GetPlotYFromXResponse,
    InstantiateMsg, QueryMsg,
};
use andromeda_std::{
    ado_base::{
        permissioning::{LocalPermission, Permission},
        InstantiateMsg as BaseInstantiateMsg, MigrateMsg,
    },
    ado_contract::ADOContract,
    common::{actions::call_action, context::ExecuteContext, encode_binary},
    error::ContractError,
};

use cosmwasm_std::{
    entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError, Storage,
};

use cw_utils::nonpayable;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-curve";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const UPDATE_CURVE_CONFIG_ACTION: &str = "update_curve_config";
pub const RESET_ACTION: &str = "reset";

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
            kernel_address: msg.kernel_address,
            owner: msg.owner,
        },
    )?;

    if let Some(authorized_operator_addresses) = msg.authorized_operator_addresses {
        if !authorized_operator_addresses.is_empty() {
            ADOContract::default().permission_action(UPDATE_CURVE_CONFIG_ACTION, deps.storage)?;
            ADOContract::default().permission_action(RESET_ACTION, deps.storage)?;
        }

        for address in authorized_operator_addresses {
            let addr = address.get_raw_address(&deps.as_ref())?;
            ADOContract::set_permission(
                deps.storage,
                UPDATE_CURVE_CONFIG_ACTION,
                addr.clone(),
                Permission::Local(LocalPermission::whitelisted(None, None)),
            )?;
            ADOContract::set_permission(
                deps.storage,
                RESET_ACTION,
                addr.clone(),
                Permission::Local(LocalPermission::whitelisted(None, None)),
            )?;
        }
    }

    msg.curve_config.validate()?;
    CURVE_CONFIG.save(deps.storage, &msg.curve_config)?;

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

fn handle_execute(mut ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    let action_response = call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;

    let res = match msg.clone() {
        ExecuteMsg::UpdateCurveConfig { curve_config } => {
            execute_update_curve_config(ctx, curve_config)
        }
        ExecuteMsg::Reset {} => execute_reset(ctx),
        _ => ADOContract::default().execute(ctx, msg),
    }?;

    Ok(res
        .add_submessages(action_response.messages)
        .add_attributes(action_response.attributes)
        .add_events(action_response.events))
}

pub fn execute_update_curve_config(
    mut ctx: ExecuteContext,
    curve_config: CurveConfig,
) -> Result<Response, ContractError> {
    nonpayable(&ctx.info)?;
    let sender = ctx.info.sender.clone();
    ADOContract::default().is_permissioned(
        ctx.deps.branch(),
        ctx.env.clone(),
        UPDATE_CURVE_CONFIG_ACTION,
        sender.clone(),
    )?;

    curve_config.validate()?;
    CURVE_CONFIG.save(ctx.deps.storage, &curve_config)?;

    Ok(Response::new()
        .add_attribute("method", "update_curve_config")
        .add_attribute("sender", sender))
}

pub fn execute_reset(mut ctx: ExecuteContext) -> Result<Response, ContractError> {
    nonpayable(&ctx.info)?;
    let sender = ctx.info.sender.clone();
    ADOContract::default().is_permissioned(
        ctx.deps.branch(),
        ctx.env.clone(),
        RESET_ACTION,
        sender.clone(),
    )?;

    CURVE_CONFIG.remove(ctx.deps.storage);

    Ok(Response::new().add_attribute("method", "reset"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetCurveConfig {} => encode_binary(&query_curve_config(deps.storage)?),
        QueryMsg::GetPlotYFromX { x_value } => {
            encode_binary(&query_plot_y_from_x(deps.storage, x_value)?)
        }
        _ => ADOContract::default().query(deps, env, msg),
    }
}

pub fn query_curve_config(storage: &dyn Storage) -> Result<GetCurveConfigResponse, ContractError> {
    let curve_config = CURVE_CONFIG.load(storage)?;
    Ok(GetCurveConfigResponse { curve_config })
}

pub fn query_plot_y_from_x(
    storage: &dyn Storage,
    x_value: f64,
) -> Result<GetPlotYFromXResponse, ContractError> {
    let curve_config = CURVE_CONFIG.load(storage)?;

    let y_value = match curve_config {
        CurveConfig::ExpConfig {
            curve_type,
            base_value,
            multiple_variable_value,
            constant_value,
        } => {
            let curve_id_f64 = match curve_type {
                CurveType::Growth => 1_f64,
                CurveType::Decay => -1_f64,
            };
            let base_value_f64 = base_value as f64;
            let constant_value_f64 = constant_value.unwrap_or(DEFAULT_CONSTANT_VALUE) as f64;
            let multiple_variable_value_f64 =
                multiple_variable_value.unwrap_or(DEFAULT_MULTIPLE_VARIABLE_VALUE) as f64;

            (constant_value_f64
                * base_value_f64.powf(curve_id_f64 * multiple_variable_value_f64 * x_value))
            .to_string()
        }
    };

    Ok(GetPlotYFromXResponse { y_value })
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
