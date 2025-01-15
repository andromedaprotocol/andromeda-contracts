use andromeda_std::{
    ado_contract::ADOContract,
    common::{
        actions::call_action, context::ExecuteContext, withdraw::WithdrawalType, Milliseconds,
    },
    error::ContractError,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure, Binary, Coin, Decimal, Deps, DepsMut, Env, MessageInfo, QuerierWrapper, Reply,
    Response, StdError, Uint128,
};
use cw_asset::AssetInfo;
use cw_utils::nonpayable;
use std::cmp;

use crate::state::{
    batches, get_all_batches_with_ids, get_claimable_batches_with_ids, save_new_batch, Batch,
    CONFIG,
};
use andromeda_finance::vesting::{BatchResponse, Config, ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    common::encode_binary,
};

const CONTRACT_NAME: &str = "crates.io:andromeda-vesting";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        recipient: msg.recipient.clone(),
        denom: msg.denom.clone(),
    };

    CONFIG.save(deps.storage, &config)?;

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

    msg.validate(&deps)?;

    Ok(inst_resp)
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
    call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;
    match msg {
        ExecuteMsg::CreateBatch {
            lockup_duration,
            release_duration,
            release_amount,
        } => execute_create_batch(ctx, lockup_duration, release_duration, release_amount),
        ExecuteMsg::Claim {
            number_of_claims,
            batch_id,
        } => execute_claim(ctx, number_of_claims, batch_id),
        ExecuteMsg::ClaimAll { limit, up_to_time } => execute_claim_all(ctx, limit, up_to_time),

        _ => ADOContract::default().execute(ctx, msg),
    }
}

fn execute_create_batch(
    ctx: ExecuteContext,
    lockup_duration: Option<Milliseconds>,
    release_duration: Milliseconds,
    release_amount: WithdrawalType,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    ensure!(
        ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let config = CONFIG.load(deps.storage)?;
    let current_time = Milliseconds::from_seconds(env.block.time.seconds());

    ensure!(
        info.funds.len() == 1,
        ContractError::InvalidFunds {
            msg: "Creating a batch must be accompanied with a single native fund".to_string(),
        }
    );

    let funds = info.funds[0].clone();

    ensure!(
        funds.denom == config.denom,
        ContractError::InvalidFunds {
            msg: "Invalid denom".to_string(),
        }
    );

    ensure!(
        !release_duration.is_zero() && !release_amount.is_zero(),
        ContractError::InvalidZeroAmount {}
    );
    ensure!(
        !release_amount.get_amount(funds.amount)?.is_zero(),
        ContractError::InvalidZeroAmount {}
    );

    let min_fund = match release_amount {
        WithdrawalType::Amount(amount) => amount,
        WithdrawalType::Percentage(_) => Uint128::from(100u128),
    };
    ensure!(
        funds.amount >= min_fund,
        ContractError::InvalidFunds {
            msg: format!("Funds must be at least {min_fund}"),
        }
    );

    let current_balance = deps
        .querier
        .query_balance(env.contract.address.to_string(), funds.denom)
        .unwrap()
        .amount;
    let max_fund = Uint128::MAX - current_balance;
    ensure!(
        funds.amount <= max_fund,
        ContractError::InvalidFunds {
            msg: format!("Funds can not exceed {max_fund}"),
        }
    );

    let lockup_end = if let Some(duration) = lockup_duration {
        current_time.plus_milliseconds(duration)
    } else {
        current_time
    };

    let release_amount_string = format!("{release_amount:?}");

    let batch = Batch {
        amount: funds.amount,
        amount_claimed: Uint128::zero(),
        lockup_end,
        release_duration,
        release_amount,
        last_claimed_release_time: lockup_end,
    };

    save_new_batch(deps.storage, batch)?;

    Ok(Response::new()
        .add_attribute("action", "create_batch")
        .add_attribute("amount", funds.amount)
        .add_attribute("lockup_end", lockup_end.to_string())
        .add_attribute("release_duration", release_duration.to_string())
        .add_attribute("release_amount", release_amount_string))
}

fn execute_claim(
    ctx: ExecuteContext,
    number_of_claims: Option<u64>,
    batch_id: u64,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    let contract = ADOContract::default();
    // Should this be owner or recipient?
    ensure!(
        contract.is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let config = CONFIG.load(deps.storage)?;

    // If it doesn't exist, error will be returned to user.
    let key = batches().key(batch_id);
    let mut batch = key.load(deps.storage)?;
    let amount_to_send = claim_batch(&deps.querier, &env, &mut batch, &config, number_of_claims)?;

    ensure!(
        !amount_to_send.is_zero(),
        ContractError::WithdrawalIsEmpty {}
    );

    key.save(deps.storage, &batch)?;

    let config = CONFIG.load(deps.storage)?;
    let withdraw_msg = config.recipient.generate_direct_msg(
        &deps.as_ref(),
        vec![Coin::new(amount_to_send.u128(), config.denom)],
    )?;

    Ok(Response::new()
        .add_submessage(withdraw_msg)
        .add_attribute("action", "claim")
        .add_attribute("amount", amount_to_send)
        .add_attribute("batch_id", batch_id.to_string())
        .add_attribute("amount_left", batch.amount - batch.amount_claimed))
}

fn execute_claim_all(
    ctx: ExecuteContext,
    limit: Option<u32>,
    up_to_time: Option<Milliseconds>,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    nonpayable(&info)?;

    let contract = ADOContract::default();

    ensure!(
        contract.is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let config = CONFIG.load(deps.storage)?;

    let current_time = Milliseconds::from_seconds(env.block.time.seconds());
    let batches_with_ids = get_claimable_batches_with_ids(deps.storage, current_time, limit)?;
    let up_to_time = Milliseconds(cmp::min(
        current_time.milliseconds(),
        up_to_time.unwrap_or(current_time).milliseconds(),
    ));

    let mut total_amount_to_send = Uint128::zero();
    let last_batch_id = if !batches_with_ids.is_empty() {
        batches_with_ids.last().unwrap().0.to_string()
    } else {
        "none".to_string()
    };
    for (batch_id, mut batch) in batches_with_ids {
        let key = batches().key(batch_id);

        let elapsed_time = up_to_time.minus_milliseconds(batch.last_claimed_release_time);
        let num_available_claims =
            elapsed_time.milliseconds() / batch.release_duration.milliseconds();

        let amount_to_send = claim_batch(
            &deps.querier,
            &env,
            &mut batch,
            &config,
            Some(num_available_claims),
        )?;

        total_amount_to_send = total_amount_to_send.checked_add(amount_to_send)?;

        key.save(deps.storage, &batch)?;
    }
    let mut msgs = vec![];

    // Don't want to error here since there will generally be other batches that will have
    // claimable amounts. Erroring for one would make the whole transaction fai.
    if !total_amount_to_send.is_zero() {
        let config = CONFIG.load(deps.storage)?;
        msgs.push(config.recipient.generate_direct_msg(
            &deps.as_ref(),
            vec![Coin::new(total_amount_to_send.u128(), config.denom)],
        )?)
    }
    Ok(Response::new()
        .add_submessages(msgs)
        .add_attribute("action", "claim_all")
        .add_attribute("last_batch_id_processed", last_batch_id))
}

fn claim_batch(
    querier: &QuerierWrapper,
    env: &Env,
    batch: &mut Batch,
    config: &Config,
    number_of_claims: Option<u64>,
) -> Result<Uint128, ContractError> {
    let current_time = Milliseconds::from_seconds(env.block.time.seconds());
    ensure!(
        batch.lockup_end <= current_time,
        ContractError::FundsAreLocked {}
    );
    let total_amount = AssetInfo::native(config.denom.to_owned())
        .query_balance(querier, env.contract.address.to_owned())?;

    let elapsed_time = current_time.minus_milliseconds(batch.last_claimed_release_time);
    let num_available_claims = elapsed_time.milliseconds() / batch.release_duration.milliseconds();

    let number_of_claims = cmp::min(
        number_of_claims.unwrap_or(num_available_claims),
        num_available_claims,
    );

    let amount_per_claim = batch.release_amount.get_amount(batch.amount)?;

    let amount_to_send = amount_per_claim
        .checked_mul(Decimal::from_ratio(number_of_claims, Uint128::one()))?
        .to_uint_floor();
    let amount_available = cmp::min(batch.amount - batch.amount_claimed, total_amount);

    let amount_to_send = cmp::min(amount_to_send, amount_available);

    // We dont want to update the last_claim_time when there are no funds to claim.
    if !amount_to_send.is_zero() {
        batch.amount_claimed = batch.amount_claimed.checked_add(amount_to_send)?;

        // Safe math version
        let claims_release_duration =
            number_of_claims.checked_mul(batch.release_duration.milliseconds());
        if claims_release_duration.is_none() {
            return Err(ContractError::Overflow {});
        }

        let claims_release_duration = Milliseconds(claims_release_duration.unwrap());

        batch.last_claimed_release_time = batch
            .last_claimed_release_time
            .plus_milliseconds(claims_release_duration);

        // The unsafe version
        // batch.last_claimed_release_time += number_of_claims * batch.release_duration;
    }

    Ok(amount_to_send)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, env, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Config {} => encode_binary(&query_config(deps)?),
        QueryMsg::Batch { id } => encode_binary(&query_batch(deps, env, id)?),
        QueryMsg::Batches { start_after, limit } => {
            encode_binary(&query_batches(deps, env, start_after, limit)?)
        }
        _ => ADOContract::default().query(deps, env, msg),
    }
}

fn query_config(deps: Deps) -> Result<Config, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config)
}

fn query_batch(deps: Deps, env: Env, batch_id: u64) -> Result<BatchResponse, ContractError> {
    let batch = batches().load(deps.storage, batch_id)?;

    let config = CONFIG.load(deps.storage)?;
    get_batch_response(&deps.querier, &env, &config, batch, batch_id)
}

fn query_batches(
    deps: Deps,
    env: Env,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> Result<Vec<BatchResponse>, ContractError> {
    let batches_with_ids = get_all_batches_with_ids(deps.storage, start_after, limit)?;
    let mut batches_response = vec![];
    let config = CONFIG.load(deps.storage)?;
    for (id, batch) in batches_with_ids {
        let batch_response = get_batch_response(&deps.querier, &env, &config, batch, id)?;

        batches_response.push(batch_response);
    }
    Ok(batches_response)
}

fn get_batch_response(
    querier: &QuerierWrapper,
    env: &Env,
    config: &Config,
    mut batch: Batch,
    batch_id: u64,
) -> Result<BatchResponse, ContractError> {
    let previous_amount = batch.amount_claimed;
    let previous_last_claimed_release_time = batch.last_claimed_release_time;
    let amount_available_to_claim = if env.block.time.seconds() >= batch.lockup_end.seconds() {
        claim_batch(querier, env, &mut batch, config, None)?
    } else {
        Uint128::zero()
    };
    let amount_per_release = batch.release_amount.get_amount(batch.amount)?;
    let number_of_available_claims = Decimal::from_ratio(amount_available_to_claim, Uint128::one())
        .checked_div(amount_per_release)
        .unwrap()
        .to_uint_floor();
    let res = BatchResponse {
        id: batch_id,
        amount: batch.amount,
        amount_claimed: previous_amount,
        amount_available_to_claim,
        number_of_available_claims,
        lockup_end: batch.lockup_end,
        release_amount: batch.release_amount,
        release_duration: batch.release_duration,
        last_claimed_release_time: previous_last_claimed_release_time,
    };

    Ok(res)
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
