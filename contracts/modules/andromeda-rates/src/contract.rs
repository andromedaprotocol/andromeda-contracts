#[cfg(not(feature = "library"))]
use crate::state::RATES;
use andromeda_modules::rates::{ExecuteMsg, InstantiateMsg, QueryMsg, RateResponse};
use andromeda_std::{
    ado_base::{
        rates::{calculate_fee, LocalRate, PaymentAttribute, RatesResponse},
        InstantiateMsg as BaseInstantiateMsg, MigrateMsg,
    },
    ado_contract::ADOContract,
    common::{context::ExecuteContext, deduct_funds, encode_binary, Funds},
    error::ContractError,
};

use cosmwasm_std::{
    attr, coin, ensure, Binary, Coin, Deps, DepsMut, Env, Event, MessageInfo, Reply, Response,
    StdError, SubMsg,
};
use cosmwasm_std::{entry_point, from_json};
use cw20::Cw20Coin;
use cw_utils::nonpayable;
// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-rates";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let action = msg.action;
    let rate = msg.rate;

    let inst_resp = ADOContract::default().instantiate(
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

    let local_rate = rate.validate(deps.as_ref())?;
    RATES.save(deps.storage, &action, &local_rate)?;

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

pub fn handle_execute(ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SetRate { action, rate } => execute_set_rate(ctx, action, rate),
        ExecuteMsg::RemoveRate { action } => execute_remove_rate(ctx, action),
        _ => ADOContract::default().execute(ctx, msg),
    }
}

fn execute_set_rate(
    ctx: ExecuteContext,
    action: String,
    rate: LocalRate,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;
    nonpayable(&info)?;

    ensure!(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    rate.validate(deps.as_ref())?;

    RATES.save(deps.storage, &action, &rate)?;

    Ok(Response::new().add_attributes(vec![attr("action", "set_rate")]))
}

fn execute_remove_rate(ctx: ExecuteContext, action: String) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;
    nonpayable(&info)?;

    ensure!(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    if RATES.has(deps.storage, &action) {
        RATES.remove(deps.storage, &action);
        Ok(Response::new().add_attributes(vec![attr("action", "remove_rates")]))
    } else {
        Err(ContractError::ActionNotFound {})
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, env, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Rate { action } => encode_binary(&query_rate(deps, action)?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

fn query_rate(deps: Deps, action: String) -> Result<RateResponse, ContractError> {
    let rate = RATES.may_load(deps.storage, &action)?;
    match rate {
        Some(rate) => Ok(RateResponse { rate }),
        None => Err(ContractError::InvalidRate {}),
    }
}

//NOTE Currently set as pub for testing
pub fn query_deducted_funds(
    deps: Deps,
    payload: Binary,
    funds: Funds,
) -> Result<RatesResponse, ContractError> {
    let action: String = from_json(payload)?;
    let local_rate = RATES.load(deps.storage, &action)?;
    let mut msgs: Vec<SubMsg> = vec![];
    let mut events: Vec<Event> = vec![];
    let (coin, is_native): (Coin, bool) = match funds {
        Funds::Native(coin) => (coin, true),
        Funds::Cw20(cw20_coin) => (coin(cw20_coin.amount.u128(), cw20_coin.address), false),
    };
    let mut leftover_funds = vec![coin.clone()];

    let event_name = if local_rate.rate_type.is_additive() {
        "tax"
    } else {
        "royalty"
    };
    let mut event = Event::new(event_name);
    if let Some(desc) = &local_rate.description {
        event = event.add_attribute("description", desc);
    }
    local_rate.value.validate(deps)?;
    let fee = calculate_fee(local_rate.value, &coin)?;

    if !local_rate.rate_type.is_additive() {
        deduct_funds(&mut leftover_funds, &fee)?;
        event = event.add_attribute("deducted", fee.to_string());
    }
    event = event.add_attribute(
        "payment",
        PaymentAttribute {
            receiver: local_rate
                .recipient
                .address
                .get_raw_address(&deps)?
                .to_string(),
            amount: fee.clone(),
        }
        .to_string(),
    );
    let msg = if is_native {
        local_rate
            .recipient
            .generate_direct_msg(&deps, vec![fee.clone()])?
    } else {
        local_rate.recipient.generate_msg_cw20(
            &deps,
            Cw20Coin {
                amount: fee.amount,
                address: fee.denom.to_string(),
            },
        )?
    };
    msgs.push(msg);

    events.push(event);

    Ok(RatesResponse {
        msgs,
        leftover_funds: if is_native {
            Funds::Native(leftover_funds[0].clone())
        } else {
            Funds::Cw20(Cw20Coin {
                amount: leftover_funds[0].amount,
                address: coin.denom,
            })
        },
        events,
    })
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
