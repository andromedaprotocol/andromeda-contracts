#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, ensure, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError, Storage,
};
use cw_utils::Expiration;

use crate::state::{CYCLE_START_TIME, GATE_ADDRESSES, TIME_INTERVAL};
use andromeda_math::time_gate::{ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    ado_contract::ADOContract,
    amp::AndrAddr,
    common::{
        actions::call_action,
        context::ExecuteContext,
        encode_binary,
        expiration::{get_and_validate_start_time, Expiry},
        Milliseconds,
    },
    error::ContractError,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-time-gate";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_TIME_INTERVAL: u64 = 3600;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let resp = ADOContract::default().instantiate(
        deps.storage,
        env.clone(),
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

    let cycle_start_time_milliseconds = match msg.cycle_start_time.clone() {
        None => Milliseconds::from_nanos(env.block.time.nanos()),
        Some(start_time) => start_time.get_time(&env.block),
    };

    let (cycle_start_time, _) = get_and_validate_start_time(&env, msg.cycle_start_time)?;

    let time_interval_seconds = msg.time_interval.unwrap_or(DEFAULT_TIME_INTERVAL);

    GATE_ADDRESSES.save(deps.storage, &msg.gate_addresses)?;
    CYCLE_START_TIME.save(
        deps.storage,
        &(cycle_start_time, cycle_start_time_milliseconds),
    )?;
    TIME_INTERVAL.save(deps.storage, &time_interval_seconds)?;

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
    let action = msg.as_ref().to_string();

    let action_response = call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;

    let res = match msg {
        ExecuteMsg::UpdateCycleStartTime { cycle_start_time } => {
            execute_update_cycle_start_time(ctx, cycle_start_time, action)
        }
        ExecuteMsg::UpdateGateAddresses { new_gate_addresses } => {
            execute_update_gate_addresses(ctx, new_gate_addresses, action)
        }
        ExecuteMsg::UpdateTimeInterval { time_interval } => {
            execute_update_time_interval(ctx, time_interval, action)
        }
        _ => ADOContract::default().execute(ctx, msg),
    }?;

    Ok(res
        .add_submessages(action_response.messages)
        .add_attributes(action_response.attributes)
        .add_events(action_response.events))
}

fn execute_update_cycle_start_time(
    ctx: ExecuteContext,
    cycle_start_time: Option<Expiry>,
    action: String,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;

    ensure!(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let (new_cycle_start_time, _) =
        get_and_validate_start_time(&ctx.env, cycle_start_time.clone())?;
    let new_cycle_start_time_milliseconds = match cycle_start_time.clone() {
        None => Milliseconds::from_nanos(ctx.env.block.time.nanos()),
        Some(start_time) => start_time.get_time(&ctx.env.block),
    };

    let (old_cycle_start_time, _) = CYCLE_START_TIME.load(deps.storage)?;

    ensure!(
        old_cycle_start_time != new_cycle_start_time,
        ContractError::InvalidParameter {
            error: Some("Same as an existed cycle start time".to_string())
        }
    );

    CYCLE_START_TIME.save(
        deps.storage,
        &(new_cycle_start_time, new_cycle_start_time_milliseconds),
    )?;

    Ok(Response::new().add_attributes(vec![attr("action", action), attr("sender", info.sender)]))
}

fn execute_update_gate_addresses(
    ctx: ExecuteContext,
    new_gate_addresses: Vec<AndrAddr>,
    action: String,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;

    ensure!(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let old_gate_addresses = GATE_ADDRESSES.load(deps.storage)?;

    ensure!(
        old_gate_addresses != new_gate_addresses,
        ContractError::InvalidParameter {
            error: Some("Same as existed gate addresses".to_string())
        }
    );

    GATE_ADDRESSES.save(deps.storage, &new_gate_addresses)?;

    Ok(Response::new().add_attributes(vec![attr("action", action), attr("sender", info.sender)]))
}

fn execute_update_time_interval(
    ctx: ExecuteContext,
    time_interval: u64,
    action: String,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;

    ensure!(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let old_time_interval = TIME_INTERVAL.load(deps.storage)?;

    ensure!(
        old_time_interval != time_interval,
        ContractError::InvalidParameter {
            error: Some("Same as an existed time interval".to_string())
        }
    );

    TIME_INTERVAL.save(deps.storage, &time_interval)?;

    Ok(Response::new().add_attributes(vec![attr("action", action), attr("sender", info.sender)]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetGateAddresses {} => encode_binary(&get_gate_addresses(deps.storage)?),
        QueryMsg::GetCycleStartTime {} => encode_binary(&get_cycle_start_time(deps.storage)?),
        QueryMsg::GetCurrentAdoPath {} => encode_binary(&get_current_ado_path(deps, env)?),
        QueryMsg::GetTimeInterval {} => encode_binary(&get_time_interval(deps.storage)?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

pub fn get_gate_addresses(storage: &dyn Storage) -> Result<Vec<AndrAddr>, ContractError> {
    let gate_addresses = GATE_ADDRESSES.load(storage)?;
    Ok(gate_addresses)
}

pub fn get_cycle_start_time(
    storage: &dyn Storage,
) -> Result<(Expiration, Milliseconds), ContractError> {
    let (cycle_start_time, cycle_start_time_milliseconds) = CYCLE_START_TIME.load(storage)?;
    Ok((cycle_start_time, cycle_start_time_milliseconds))
}

pub fn get_time_interval(storage: &dyn Storage) -> Result<String, ContractError> {
    let time_interval = TIME_INTERVAL.load(storage)?.to_string();
    Ok(time_interval)
}

pub fn get_current_ado_path(deps: Deps, env: Env) -> Result<Addr, ContractError> {
    let storage = deps.storage;
    let (cycle_start_time, cycle_start_time_milliseconds) = CYCLE_START_TIME.load(storage)?;
    let gate_addresses = GATE_ADDRESSES.load(storage)?;
    let time_interval = TIME_INTERVAL.load(storage)?;

    ensure!(
        cycle_start_time.is_expired(&env.block),
        ContractError::CustomError {
            msg: "Cycle is not started yet".to_string()
        }
    );

    let current_time_nanos = env.block.time.nanos();
    let cycle_start_nanos = cycle_start_time_milliseconds.nanos();

    let time_interval_nanos = match time_interval.checked_mul(1_000_000_000) {
        Some(val) => val,
        None => return Err(ContractError::Overflow {}),
    };
    let gate_length = gate_addresses.len() as u64;
    let time_delta = match current_time_nanos.checked_sub(cycle_start_nanos) {
        Some(val) => val,
        None => return Err(ContractError::Underflow {}),
    };

    let index = match time_delta.checked_div(time_interval_nanos) {
        Some(val) => val,
        None => {
            return Err(ContractError::CustomError {
                msg: "Division by zero in time delta".to_string(),
            })
        }
    };
    let index = match index.checked_rem(gate_length) {
        Some(val) => val as usize,
        None => {
            return Err(ContractError::CustomError {
                msg: "Modulo by zero in gate length".to_string(),
            })
        }
    };

    let current_ado_path = &gate_addresses[index];
    let result = current_ado_path.get_raw_address(&deps)?;

    Ok(result)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, CONTRACT_NAME, CONTRACT_VERSION)
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
