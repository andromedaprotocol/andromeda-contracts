#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, ensure, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError, Storage,
};

use andromeda_math::graph::{
    Coordinate, CoordinateInfo, ExecuteMsg, GetAllPointsResponse, GetMapInfoResponse,
    GetMaxPointNumberResponse, InstantiateMsg, MapInfo, QueryMsg, StoredDate,
};
use andromeda_math::point::{GetDataOwnerResponse, PointCoordinate, QueryMsg as PointQueryMsg};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    ado_contract::ADOContract,
    amp::AndrAddr,
    common::{actions::call_action, context::ExecuteContext, encode_binary},
    error::ContractError,
    os::aos_querier::AOSQuerier,
};

use crate::state::{MAP_INFO, MAP_POINT_INFO, TOTAL_POINTS_NUMBER, USER_COORDINATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-graph";
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

    MAP_INFO.save(deps.storage, &msg.map_info)?;
    TOTAL_POINTS_NUMBER.save(deps.storage, &0)?;

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

    let res = match msg {
        ExecuteMsg::UpdateMap { map_info } => execute_update_map(ctx, map_info),
        ExecuteMsg::StoreCoordinate {
            coordinate,
            is_timestamp_allowed,
        } => execute_store_coordinate(ctx, coordinate, is_timestamp_allowed),
        ExecuteMsg::StoreUserCoordinate {
            user_location_paths,
        } => execute_store_user_coordinate(ctx, user_location_paths),
        ExecuteMsg::DeleteUserCoordinate { user } => execute_delete_user_coordinate(ctx, user),
        _ => ADOContract::default().execute(ctx, msg),
    }?;

    Ok(res
        .add_submessages(action_response.messages)
        .add_attributes(action_response.attributes)
        .add_events(action_response.events))
}

pub fn execute_update_map(
    ctx: ExecuteContext,
    map_info: MapInfo,
) -> Result<Response, ContractError> {
    let sender = ctx.info.sender;
    ensure!(
        ADOContract::default().is_owner_or_operator(ctx.deps.storage, sender.as_ref())?,
        ContractError::Unauthorized {}
    );

    let map = MAP_INFO
        .load(ctx.deps.storage)
        .map_err(|_| ContractError::InvalidParameter {
            error: Some("Map not found".to_string()),
        })?;

    ensure!(
        map != map_info,
        ContractError::InvalidParameter {
            error: Some("Map already exists".to_string())
        }
    );

    MAP_INFO.save(ctx.deps.storage, &map_info)?;

    let max_point_number = TOTAL_POINTS_NUMBER.load(ctx.deps.storage)?;

    for point in 1..=max_point_number {
        MAP_POINT_INFO.remove(ctx.deps.storage, &point);
    }

    TOTAL_POINTS_NUMBER.save(ctx.deps.storage, &0)?;

    Ok(Response::new().add_attributes(vec![attr("method", "update_map"), attr("sender", sender)]))
}

pub fn execute_store_coordinate(
    ctx: ExecuteContext,
    coordinate: Coordinate,
    is_timestamp_allowed: bool,
) -> Result<Response, ContractError> {
    let sender = ctx.info.sender;
    ensure!(
        ADOContract::default().is_owner_or_operator(ctx.deps.storage, sender.as_ref())?,
        ContractError::Unauthorized {}
    );

    let map = MAP_INFO
        .load(ctx.deps.storage)
        .map_err(|_| ContractError::InvalidParameter {
            error: Some("Map not found".to_string()),
        })?;

    let MapInfo {
        map_size,
        allow_negative,
        map_decimal,
    } = map;
    let x_length = map_size.x_width as f64;
    let y_length = map_size.y_width as f64;
    let z_length = map_size.z_width.map(|z| z as f64);

    let is_z_allowed = z_length.is_some();

    let scale = 10_f64.powf(map_decimal as f64);
    let x_coordinate = (coordinate.x_coordinate * scale)
        .min(i64::MAX as f64) // Ensuring it doesn't exceed i64 bounds
        .max(i64::MIN as f64); // Ensuring it doesn't underflow
    let x_coordinate = (x_coordinate as i64) as f64 / scale;

    let y_coordinate = (coordinate.y_coordinate * scale)
        .min(i64::MAX as f64) // Ensuring it doesn't exceed i64 bounds
        .max(i64::MIN as f64); // Ensuring it doesn't underflow
    let y_coordinate = (y_coordinate as i64) as f64 / scale;

    let z_coordinate = coordinate.z_coordinate.map(|z| {
        let z_scaled = (z * scale)
            .min(i64::MAX as f64) // Clamp the value to prevent overflow
            .max(i64::MIN as f64); // Clamp the value to prevent underflow
        (z_scaled as i64) as f64 / scale
    });

    ensure!(
        z_coordinate.is_some() == is_z_allowed,
        ContractError::InvalidParameter {
            error: Some(if is_z_allowed {
                "Z-axis is allowed".to_string()
            } else {
                "Z-axis is not allowed".to_string()
            })
        }
    );

    match allow_negative {
        true => {
            ensure!(
                x_coordinate >= -(x_length / 2_f64) && x_coordinate <= x_length / 2_f64,
                ContractError::InvalidParameter {
                    error: Some("Wrong X Coordinate Range".to_string())
                }
            );

            ensure!(
                y_coordinate >= -(y_length / 2_f64) && y_coordinate <= y_length / 2_f64,
                ContractError::InvalidParameter {
                    error: Some("Wrong Y Coordinate Range".to_string())
                }
            );

            if is_z_allowed {
                if let Some(z_coordinate) = z_coordinate {
                    ensure!(
                        z_coordinate >= -(z_length.unwrap() / 2_f64)
                            && z_coordinate <= z_length.unwrap() / 2_f64,
                        ContractError::InvalidParameter {
                            error: Some("Wrong Z Coordinate Range".to_string())
                        }
                    );
                }
            }
        }
        false => {
            ensure!(
                x_coordinate >= 0_f64 && x_coordinate <= x_length,
                ContractError::InvalidParameter {
                    error: Some("Wrong X Coordinate Range".to_string())
                }
            );

            ensure!(
                y_coordinate >= 0_f64 && y_coordinate <= y_length,
                ContractError::InvalidParameter {
                    error: Some("Wrong Y Coordinate Range".to_string())
                }
            );

            if is_z_allowed {
                if let Some(z_coordinate) = z_coordinate {
                    ensure!(
                        z_coordinate >= 0_f64 && z_coordinate <= z_length.unwrap(),
                        ContractError::InvalidParameter {
                            error: Some("Wrong Z Coordinate Range".to_string())
                        }
                    );
                }
            }
        }
    };

    let point_number = TOTAL_POINTS_NUMBER
        .load(ctx.deps.storage)?
        .checked_add(1)
        .unwrap();
    let timestamp = match is_timestamp_allowed {
        true => Some(ctx.env.block.time.nanos()),
        false => None,
    };

    MAP_POINT_INFO.save(
        ctx.deps.storage,
        &point_number,
        &(
            CoordinateInfo {
                x: x_coordinate.to_string(),
                y: y_coordinate.to_string(),
                z: z_coordinate.map(|z_coordinate| z_coordinate.to_string()),
            },
            StoredDate { timestamp },
        ),
    )?;
    TOTAL_POINTS_NUMBER.save(ctx.deps.storage, &point_number)?;

    Ok(Response::new().add_attributes(vec![
        attr("method", "store_coordinate"),
        attr("sender", sender),
    ]))
}

pub fn execute_store_user_coordinate(
    ctx: ExecuteContext,
    user_location_paths: Vec<AndrAddr>,
) -> Result<Response, ContractError> {
    let sender = ctx.info.sender;
    ensure!(
        ADOContract::default().is_owner_or_operator(ctx.deps.storage, sender.as_ref())?,
        ContractError::Unauthorized {}
    );
    for user_location_path in user_location_paths {
        let address = user_location_path.get_raw_address(&ctx.deps.as_ref())?;
        let contract_info = ctx.deps.querier.query_wasm_contract_info(address.clone());
        if let Ok(contract_info) = contract_info {
            let code_id = contract_info.code_id;
            let adodb_addr =
                ADOContract::default().get_adodb_address(ctx.deps.storage, &ctx.deps.querier)?;
            let ado_type = AOSQuerier::ado_type_getter(&ctx.deps.querier, &adodb_addr, code_id)?;

            if ado_type.is_none() {
                return Err(ContractError::InvalidADOType {
                    msg: Some("ADO Type must be point: None".to_string()),
                });
            }
            let ado_type = ado_type.unwrap();
            if ado_type == *"point" {
                let user_point_coordinate: PointCoordinate = ctx
                    .deps
                    .querier
                    .query_wasm_smart(address.clone(), &PointQueryMsg::GetPoint {})?;
                let user_res: GetDataOwnerResponse = ctx
                    .deps
                    .querier
                    .query_wasm_smart(address.clone(), &PointQueryMsg::GetDataOwner {})?;
                let user: AndrAddr = user_res.owner;
                let user_addr = user.get_raw_address(&ctx.deps.as_ref())?;

                user_point_coordinate.validate()?;

                USER_COORDINATE.save(ctx.deps.storage, user_addr, &user_point_coordinate)?;
            } else {
                return Err(ContractError::InvalidADOType {
                    msg: Some(format!("ADO Type must be point: {:?}", ado_type)),
                });
            }
        } else {
            // Not a contract
            return Err(ContractError::InvalidAddress {});
        }
    }
    Ok(Response::new().add_attributes(vec![
        attr("method", "store_user_coordinate"),
        attr("sender", sender),
    ]))
}

pub fn execute_delete_user_coordinate(
    ctx: ExecuteContext,
    user: AndrAddr,
) -> Result<Response, ContractError> {
    let sender = ctx.info.sender;
    ensure!(
        ADOContract::default().is_owner_or_operator(ctx.deps.storage, sender.as_ref())?,
        ContractError::Unauthorized {}
    );
    let user_addr = user.get_raw_address(&ctx.deps.as_ref())?;

    USER_COORDINATE.remove(ctx.deps.storage, user_addr);

    Ok(Response::new().add_attributes(vec![
        attr("method", "delete_user_coordinate"),
        attr("sender", sender),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetMapInfo {} => encode_binary(&get_map_info(deps.storage)?),
        QueryMsg::GetMaxPointNumber {} => encode_binary(&get_max_point_number(deps.storage)?),
        QueryMsg::GetAllPoints { start, limit } => {
            encode_binary(&get_all_points(deps.storage, start, limit)?)
        }
        QueryMsg::GetUserCoordinate { user } => encode_binary(&get_user_coordinate(deps, user)?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

pub fn get_map_info(storage: &dyn Storage) -> Result<GetMapInfoResponse, ContractError> {
    let map_info = MAP_INFO
        .load(storage)
        .map_err(|_| ContractError::InvalidParameter {
            error: Some("Map not found".to_string()),
        });
    match map_info {
        Ok(map_info) => Ok(GetMapInfoResponse { map_info }),
        Err(err) => Err(err),
    }
}

pub fn get_max_point_number(
    storage: &dyn Storage,
) -> Result<GetMaxPointNumberResponse, ContractError> {
    let max_point_number = TOTAL_POINTS_NUMBER.load(storage)?;
    Ok(GetMaxPointNumberResponse { max_point_number })
}

pub fn get_all_points(
    storage: &dyn Storage,
    start: Option<u128>,
    limit: Option<u32>,
) -> Result<GetAllPointsResponse, ContractError> {
    let max_point_number = TOTAL_POINTS_NUMBER.load(storage)?;

    // Set default values for pagination
    let start_point = start.unwrap_or(1); // Start from 1 if no start provided
    let limit = limit.unwrap_or(100); // Default limit to 100 points

    let mut res: Vec<(CoordinateInfo, StoredDate)> = Vec::new();

    // Iterate with pagination
    for point in start_point..=max_point_number {
        if res.len() >= limit as usize {
            break; // Stop when limit is reached
        }

        // Use `may_load` to handle cases where the point may not exist
        if let Some(coordinate) = MAP_POINT_INFO.may_load(storage, &point)? {
            res.push(coordinate);
        }
    }

    Ok(GetAllPointsResponse { points: res })
}

pub fn get_user_coordinate(deps: Deps, user: AndrAddr) -> Result<CoordinateInfo, ContractError> {
    let user_addr = user.get_raw_address(&deps)?;
    let user_coordinate = USER_COORDINATE.load(deps.storage, user_addr)?;

    Ok(CoordinateInfo {
        x: user_coordinate.x_coordinate,
        y: user_coordinate.y_coordinate,
        z: user_coordinate.z_coordinate,
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
