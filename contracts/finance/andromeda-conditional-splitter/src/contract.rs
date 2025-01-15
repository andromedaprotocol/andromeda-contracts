use crate::state::CONDITIONAL_SPLITTER;
use andromeda_finance::conditional_splitter::{
    get_threshold, ConditionalSplitter, ExecuteMsg, GetConditionalSplitterConfigResponse,
    InstantiateMsg, QueryMsg, Threshold,
};
use std::vec;

use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    amp::messages::AMPPkt,
    common::{
        actions::call_action, encode_binary, expiration::Expiry, Milliseconds,
        MillisecondsExpiration,
    },
    error::ContractError,
};
use andromeda_std::{ado_contract::ADOContract, common::context::ExecuteContext};
use cosmwasm_std::{
    attr, ensure, entry_point, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Reply, Response, StdError, SubMsg, Uint128,
};
use cw_utils::nonpayable;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-conditional-splitter";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
// 1 day in milliseconds
const ONE_DAY: u64 = 86_400_000;
// 1 year in milliseconds
const ONE_YEAR: u64 = 31_536_000_000;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let mut conditional_splitter = ConditionalSplitter {
        thresholds: msg.thresholds.clone(),
        lock_time: MillisecondsExpiration::zero(),
    };

    if let Some(lock_time) = msg.lock_time {
        let time = lock_time.get_time(&env.block);
        // New lock time can't be too short
        ensure!(
            time >= Milliseconds::from_seconds(env.block.time.seconds())
                .plus_milliseconds(Milliseconds(ONE_DAY)),
            ContractError::LockTimeTooShort {}
        );

        // New lock time can't be too long
        ensure!(
            time <= Milliseconds::from_seconds(env.block.time.seconds())
                .plus_milliseconds(Milliseconds(ONE_YEAR)),
            ContractError::LockTimeTooLong {}
        );

        conditional_splitter.lock_time = time;
    }

    // Validate thresholds
    conditional_splitter.validate(deps.as_ref())?;

    // Save kernel address after validating it
    CONDITIONAL_SPLITTER.save(deps.storage, &conditional_splitter)?;

    let inst_resp = ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        &deps.querier,
        info,
        BaseInstantiateMsg {
            ado_type: CONTRACT_NAME.to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            kernel_address: msg.kernel_address.clone(),
            owner: msg.owner.clone(),
        },
    )?;

    Ok(inst_resp)
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
    let ctx = ExecuteContext::new(deps, info, env);

    match msg {
        ExecuteMsg::AMPReceive(pkt) => {
            ADOContract::default().execute_amp_receive(ctx, pkt, handle_execute)
        }
        _ => handle_execute(ctx, msg),
    }
}

pub fn handle_execute(mut ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    let action_response = call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;
    let res = match msg {
        ExecuteMsg::UpdateThresholds { thresholds } => execute_update_thresholds(ctx, thresholds),
        ExecuteMsg::UpdateLock { lock_time } => execute_update_lock(ctx, lock_time),
        ExecuteMsg::Send {} => execute_send(ctx),
        _ => ADOContract::default().execute(ctx, msg),
    }?;
    Ok(res
        .add_submessages(action_response.messages)
        .add_attributes(action_response.attributes)
        .add_events(action_response.events))
}

fn execute_send(ctx: ExecuteContext) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;

    ensure!(
        !info.funds.is_empty(),
        ContractError::InvalidFunds {
            msg: "At least one coin should to be sent".to_string(),
        }
    );
    ensure!(
        info.funds.len() < 5,
        ContractError::ExceedsMaxAllowedCoins {}
    );
    for coin in info.funds.clone() {
        ensure!(
            !coin.amount.is_zero(),
            ContractError::InvalidFunds {
                msg: "Amount must be non-zero".to_string(),
            }
        );
    }

    let conditional_splitter = CONDITIONAL_SPLITTER.load(deps.storage)?;

    let mut msgs: Vec<SubMsg> = Vec::new();
    let mut amp_funds: Vec<Coin> = Vec::new();

    let mut remainder_funds = info.funds.clone();

    let mut pkt = AMPPkt::from_ctx(ctx.amp_ctx, ctx.env.contract.address.to_string());

    for (i, coin) in info.funds.clone().iter().enumerate() {
        // Find the relevant threshold
        let threshold = get_threshold(&conditional_splitter.thresholds, coin.amount)?;

        for address_percent in threshold.address_percent {
            let recipient_percent = address_percent.percent;
            let amount_owed = coin.amount.mul_floor(recipient_percent);

            if !amount_owed.is_zero() {
                let mut vec_coin: Vec<Coin> = Vec::new();
                let mut recip_coin: Coin = coin.clone();

                recip_coin.amount = amount_owed;

                remainder_funds[i].amount =
                    remainder_funds[i].amount.checked_sub(recip_coin.amount)?;
                vec_coin.push(recip_coin.clone());
                amp_funds.push(recip_coin);

                let amp_msg = address_percent
                    .recipient
                    .generate_amp_msg(&deps.as_ref(), Some(vec_coin))?;
                pkt = pkt.add_message(amp_msg);
            }
        }
    }

    remainder_funds.retain(|x| x.amount > Uint128::zero());

    if !remainder_funds.is_empty() {
        msgs.push(SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: remainder_funds,
        })));
    }
    if !pkt.messages.is_empty() {
        let kernel_address = ADOContract::default().get_kernel_address(deps.as_ref().storage)?;
        let distro_msg = pkt.to_sub_msg(kernel_address, Some(amp_funds), 1)?;
        msgs.push(distro_msg);
    }

    Ok(Response::new()
        .add_submessages(msgs)
        .add_attribute("action", "send")
        .add_attribute("sender", info.sender.to_string()))
}

fn execute_update_thresholds(
    ctx: ExecuteContext,
    thresholds: Vec<Threshold>,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    nonpayable(&info)?;

    ensure!(
        ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let conditional_splitter = CONDITIONAL_SPLITTER.load(deps.storage)?;

    // Can't call this function while the lock isn't expired
    ensure!(
        conditional_splitter.lock_time.is_expired(&env.block),
        ContractError::ContractLocked { msg: None }
    );

    let updated_conditional_splitter = ConditionalSplitter {
        thresholds,
        lock_time: conditional_splitter.lock_time,
    };
    // Validate the updated conditional splitter
    updated_conditional_splitter.validate(deps.as_ref())?;

    CONDITIONAL_SPLITTER.save(deps.storage, &updated_conditional_splitter)?;

    Ok(Response::default().add_attributes(vec![attr("action", "update_thresholds")]))
}

fn execute_update_lock(ctx: ExecuteContext, lock_time: Expiry) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    nonpayable(&info)?;

    ensure!(
        ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let mut conditional_splitter = CONDITIONAL_SPLITTER.load(deps.storage)?;

    // Can't call this function while the lock isn't expired
    ensure!(
        conditional_splitter.lock_time.is_expired(&env.block),
        ContractError::ContractLocked { msg: None }
    );

    let new_lock_time_expiration = lock_time.get_time(&env.block);
    // New lock time can't be too short
    ensure!(
        new_lock_time_expiration
            >= Milliseconds::from_seconds(env.block.time.seconds())
                .plus_milliseconds(Milliseconds(ONE_DAY)),
        ContractError::LockTimeTooShort {}
    );

    // New lock time can't be too long
    ensure!(
        new_lock_time_expiration
            <= Milliseconds::from_seconds(env.block.time.seconds())
                .plus_milliseconds(Milliseconds(ONE_YEAR)),
        ContractError::LockTimeTooLong {}
    );

    conditional_splitter.lock_time = new_lock_time_expiration;

    CONDITIONAL_SPLITTER.save(deps.storage, &conditional_splitter)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "update_lock"),
        attr("locked", new_lock_time_expiration.to_string()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, env, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetConditionalSplitterConfig {} => encode_binary(&query_splitter(deps)?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

fn query_splitter(deps: Deps) -> Result<GetConditionalSplitterConfigResponse, ContractError> {
    let splitter = CONDITIONAL_SPLITTER.load(deps.storage)?;

    Ok(GetConditionalSplitterConfigResponse { config: splitter })
}
