#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, ensure, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError, Storage,
};

use andromeda_math::counter::{CounterRestriction, ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_math::counter::{
    GetCurrentAmountResponse, GetDecreaseAmountResponse, GetIncreaseAmountResponse,
    GetInitialAmountResponse, GetRestrictionResponse,
};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    ado_contract::ADOContract,
    common::{context::ExecuteContext, encode_binary},
    error::ContractError,
};
use cw_utils::nonpayable;

use crate::state::{CURRENT_AMOUNT, DECREASE_AMOUNT, INCREASE_AMOUNT, INITIAL_AMOUNT, RESTRICTION};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-counter";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_INITIAL_AMOUNT: u64 = 0;
const DEFAULT_INCREASE_AMOUNT: u64 = 1;
const DEFAULT_DECREASE_AMOUNT: u64 = 1;

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

    RESTRICTION.save(deps.storage, &msg.restriction)?;

    let initial_amount = msg
        .initial_state
        .initial_amount
        .unwrap_or(DEFAULT_INITIAL_AMOUNT);
    INITIAL_AMOUNT.save(deps.storage, &initial_amount)?;
    CURRENT_AMOUNT.save(deps.storage, &initial_amount)?;

    let increase_amount = msg
        .initial_state
        .increase_amount
        .unwrap_or(DEFAULT_INCREASE_AMOUNT);
    INCREASE_AMOUNT.save(deps.storage, &increase_amount)?;

    let decrease_amount = msg
        .initial_state
        .decrease_amount
        .unwrap_or(DEFAULT_DECREASE_AMOUNT);
    DECREASE_AMOUNT.save(deps.storage, &decrease_amount)?;

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

fn handle_execute(ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    let action = msg.as_ref().to_string();

    // let action_response = call_action(
    //     &mut ctx.deps,
    //     &ctx.info,
    //     &ctx.env,
    //     &ctx.amp_ctx,
    //     msg.as_ref(),
    // )?;

    let res = match msg {
        ExecuteMsg::Increment {} => execute_increment(ctx, action),
        ExecuteMsg::Decrement {} => execute_decrement(ctx, action),
        ExecuteMsg::Reset {} => execute_reset(ctx, action),
        ExecuteMsg::UpdateRestriction { restriction } => {
            execute_update_restriction(ctx, restriction, action)
        }
        ExecuteMsg::SetIncreaseAmount { increase_amount } => {
            execute_set_increase_amount(ctx, increase_amount, action)
        }
        ExecuteMsg::SetDecreaseAmount { decrease_amount } => {
            execute_set_decrease_amount(ctx, decrease_amount, action)
        }
        _ => ADOContract::default().execute(ctx, msg),
    }?;

    Ok(
        res, // .add_submessages(action_response.messages)
            // .add_attributes(action_response.attributes)
            // .add_events(action_response.events))
    )
}

pub fn execute_increment(ctx: ExecuteContext, action: String) -> Result<Response, ContractError> {
    nonpayable(&ctx.info)?;

    let sender = ctx.info.sender.clone();
    ensure!(
        has_permission(ctx.deps.storage, &sender)?,
        ContractError::Unauthorized {}
    );

    let increase_amount = INCREASE_AMOUNT.load(ctx.deps.storage)?;
    let current_amount = CURRENT_AMOUNT
        .load(ctx.deps.storage)?
        .checked_add(increase_amount)
        .ok_or(ContractError::Overflow {})?;

    CURRENT_AMOUNT.save(ctx.deps.storage, &current_amount)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", action),
        attr("sender", sender),
        attr("current_amount", current_amount.to_string()),
    ]))
}

pub fn execute_decrement(ctx: ExecuteContext, action: String) -> Result<Response, ContractError> {
    nonpayable(&ctx.info)?;

    let sender = ctx.info.sender.clone();
    ensure!(
        has_permission(ctx.deps.storage, &sender)?,
        ContractError::Unauthorized {}
    );

    let decrease_amount = DECREASE_AMOUNT.load(ctx.deps.storage)?;
    let current_amount = CURRENT_AMOUNT
        .load(ctx.deps.storage)?
        .saturating_sub(decrease_amount);

    CURRENT_AMOUNT.save(ctx.deps.storage, &current_amount)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", action),
        attr("sender", sender),
        attr("current_amount", current_amount.to_string()),
    ]))
}

pub fn execute_reset(ctx: ExecuteContext, action: String) -> Result<Response, ContractError> {
    let sender = ctx.info.sender.clone();
    ensure!(
        has_permission(ctx.deps.storage, &sender)?,
        ContractError::Unauthorized {}
    );

    let initial_amount = INITIAL_AMOUNT.load(ctx.deps.storage)?;
    CURRENT_AMOUNT.save(ctx.deps.storage, &initial_amount)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", action),
        attr("sender", sender),
        attr("current_amount", initial_amount.to_string()),
    ]))
}

pub fn execute_update_restriction(
    ctx: ExecuteContext,
    restriction: CounterRestriction,
    action: String,
) -> Result<Response, ContractError> {
    nonpayable(&ctx.info)?;
    let sender = ctx.info.sender;
    ensure!(
        ADOContract::default().is_owner_or_operator(ctx.deps.storage, sender.as_ref())?,
        ContractError::Unauthorized {}
    );
    RESTRICTION.save(ctx.deps.storage, &restriction)?;

    Ok(Response::new().add_attributes(vec![attr("action", action), attr("sender", sender)]))
}

pub fn execute_set_increase_amount(
    ctx: ExecuteContext,
    increase_amount: u64,
    action: String,
) -> Result<Response, ContractError> {
    let sender = ctx.info.sender;
    ensure!(
        ADOContract::default().is_owner_or_operator(ctx.deps.storage, sender.as_ref())?,
        ContractError::Unauthorized {}
    );

    INCREASE_AMOUNT.save(ctx.deps.storage, &increase_amount)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", action),
        attr("sender", sender),
        attr("increase_amount", increase_amount.to_string()),
    ]))
}

pub fn execute_set_decrease_amount(
    ctx: ExecuteContext,
    decrease_amount: u64,
    action: String,
) -> Result<Response, ContractError> {
    let sender = ctx.info.sender;
    ensure!(
        ADOContract::default().is_owner_or_operator(ctx.deps.storage, sender.as_ref())?,
        ContractError::Unauthorized {}
    );

    DECREASE_AMOUNT.save(ctx.deps.storage, &decrease_amount)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", action),
        attr("sender", sender),
        attr("decrease_amount", decrease_amount.to_string()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetInitialAmount {} => encode_binary(&get_initial_amount(deps.storage)?),
        QueryMsg::GetCurrentAmount {} => encode_binary(&get_current_amount(deps.storage)?),
        QueryMsg::GetIncreaseAmount {} => encode_binary(&get_increase_amount(deps.storage)?),
        QueryMsg::GetDecreaseAmount {} => encode_binary(&get_decrease_amount(deps.storage)?),
        QueryMsg::GetRestriction {} => encode_binary(&get_restriction(deps.storage)?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

pub fn get_initial_amount(
    storage: &dyn Storage,
) -> Result<GetInitialAmountResponse, ContractError> {
    let initial_amount = INITIAL_AMOUNT.load(storage)?;
    Ok(GetInitialAmountResponse { initial_amount })
}

pub fn get_current_amount(
    storage: &dyn Storage,
) -> Result<GetCurrentAmountResponse, ContractError> {
    let current_amount = CURRENT_AMOUNT.load(storage)?;
    Ok(GetCurrentAmountResponse { current_amount })
}

pub fn get_increase_amount(
    storage: &dyn Storage,
) -> Result<GetIncreaseAmountResponse, ContractError> {
    let increase_amount = INCREASE_AMOUNT.load(storage)?;
    Ok(GetIncreaseAmountResponse { increase_amount })
}

pub fn get_decrease_amount(
    storage: &dyn Storage,
) -> Result<GetDecreaseAmountResponse, ContractError> {
    let decrease_amount = DECREASE_AMOUNT.load(storage)?;
    Ok(GetDecreaseAmountResponse { decrease_amount })
}

pub fn get_restriction(storage: &dyn Storage) -> Result<GetRestrictionResponse, ContractError> {
    let restriction = RESTRICTION.load(storage)?;
    Ok(GetRestrictionResponse { restriction })
}

pub fn has_permission(storage: &dyn Storage, addr: &Addr) -> Result<bool, ContractError> {
    let is_operator = ADOContract::default().is_owner_or_operator(storage, addr.as_str())?;
    let allowed = match RESTRICTION.load(storage)? {
        CounterRestriction::Private => is_operator,
        CounterRestriction::Public => true,
    };
    Ok(is_operator || allowed)
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
