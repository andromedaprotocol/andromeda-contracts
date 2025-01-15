#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError};

use andromeda_math::distance::{Coordinate, DistanceType, ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    ado_contract::ADOContract,
    common::{actions::call_action, context::ExecuteContext, encode_binary},
    error::ContractError,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-distance";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

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

#[allow(clippy::match_single_binding)]
fn handle_execute(mut ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    let action_response = call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;

    let res = match msg {
        _ => ADOContract::default().execute(ctx, msg),
    }?;

    Ok(res
        .add_submessages(action_response.messages)
        .add_attributes(action_response.attributes)
        .add_events(action_response.events))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetDistanceBetween2Points {
            point_1,
            point_2,
            decimal,
        } => encode_binary(&get_distance(point_1, point_2, decimal)?),
        QueryMsg::GetManhattanDistance {
            point_1,
            point_2,
            decimal,
        } => encode_binary(&get_manhattan_distance(point_1, point_2, decimal)?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

fn get_distance(
    point_1: Coordinate,
    point_2: Coordinate,
    decimal: u16,
) -> Result<String, ContractError> {
    decimal_validate(decimal)?;

    let distance = calculate_distance(point_1, point_2, decimal, DistanceType::Straight)?;

    Ok(distance)
}

fn get_manhattan_distance(
    point_1: Coordinate,
    point_2: Coordinate,
    decimal: u16,
) -> Result<String, ContractError> {
    decimal_validate(decimal)?;

    let manhattan_distance =
        calculate_distance(point_1, point_2, decimal, DistanceType::Manhattan)?;

    Ok(manhattan_distance)
}

fn calculate_distance(
    point_1: Coordinate,
    point_2: Coordinate,
    decimal: u16,
    distance_type: DistanceType,
) -> Result<String, ContractError> {
    let delta_x = (point_1.x_coordinate - point_2.x_coordinate).abs();
    let delta_y = (point_1.y_coordinate - point_2.y_coordinate).abs();
    let z_1 = point_1.z_coordinate.unwrap_or(0_f64);
    let z_2 = point_2.z_coordinate.unwrap_or(0_f64);
    let delta_z = (z_1 - z_2).abs();

    match distance_type {
        DistanceType::Straight => {
            let distance = (delta_x.powf(2_f64) + delta_y.powf(2_f64) + delta_z.powf(2_f64)).sqrt();
            let distance_decimal = format!("{:.*}", decimal as usize, distance)
                .parse::<f64>()
                .map_err(|_| ContractError::ParsingError {
                    err: "Parsing error".to_string(),
                })?;

            Ok(distance_decimal.to_string())
        }
        DistanceType::Manhattan => {
            let manhattan_distance = delta_x + delta_y + delta_z;
            let manhattan_distance_decimal = format!("{:.*}", decimal as usize, manhattan_distance)
                .parse::<f64>()
                .map_err(|_| ContractError::ParsingError {
                    err: "Parsing error".to_string(),
                })?;

            Ok(manhattan_distance_decimal.to_string())
        }
    }
}

fn decimal_validate(decimal: u16) -> Result<(), ContractError> {
    if decimal > 18 {
        return Err(ContractError::InvalidParameter {
            error: Some("Decimal value too large".to_string()),
        });
    }
    Ok(())
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
