use andromeda_finance::timelock::{
    Escrow, EscrowCondition, ExecuteMsg, GetLockedFundsForRecipientResponse,
    GetLockedFundsResponse, InstantiateMsg, QueryMsg,
};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    amp::Recipient,
    common::{actions::call_action, encode_binary},
    error::ContractError,
};
use andromeda_std::{ado_contract::ADOContract, common::context::ExecuteContext};
use cosmwasm_std::{
    attr, ensure, entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError,
    SubMsg,
};

use crate::state::{escrows, get_key, get_keys_for_recipient};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-timelock";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    let resp = contract.instantiate(
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

pub fn handle_execute(mut ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    let action_response = call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;
    let res = match msg {
        ExecuteMsg::HoldFunds {
            condition,
            recipient,
        } => execute_hold_funds(ctx, condition, recipient),
        ExecuteMsg::ReleaseFunds {
            recipient_addr,
            start_after,
            limit,
        } => execute_release_funds(ctx, recipient_addr, start_after, limit),
        ExecuteMsg::ReleaseSpecificFunds {
            owner,
            recipient_addr,
        } => execute_release_specific_funds(ctx, owner, recipient_addr),

        _ => ADOContract::default().execute(ctx, msg),
    }?;
    Ok(res
        .add_submessages(action_response.messages)
        .add_attributes(action_response.attributes)
        .add_events(action_response.events))
}

fn execute_hold_funds(
    ctx: ExecuteContext,
    condition: Option<EscrowCondition>,
    recipient: Option<Recipient>,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let rec = recipient.unwrap_or_else(|| Recipient::from_string(info.sender.to_string()));

    //Validate recipient address
    let recipient_addr = rec.clone().address;
    rec.address.validate(deps.api)?;

    let key = get_key(info.sender.as_str(), recipient_addr.as_str());
    let mut escrow = Escrow {
        coins: info.funds,
        condition,
        recipient: rec,
        recipient_addr: recipient_addr.into_string(),
    };
    // Add funds to existing escrow if it exists.
    let existing_escrow = escrows().may_load(deps.storage, key.to_vec())?;
    if let Some(existing_escrow) = existing_escrow {
        // Keep the original condition.
        escrow.condition = existing_escrow.condition;
        escrow.add_funds(existing_escrow.coins);
    } else {
        // Only want to validate if the escrow doesn't exist already. This is because it might be
        // unlocked at this point, which is fine if funds are being added to it.
        escrow.validate(deps.api, &env.block)?;
    }
    escrows().save(deps.storage, key.to_vec(), &escrow)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "hold_funds"),
        attr("sender", info.sender),
        attr("recipient", format!("{:?}", escrow.recipient)),
        attr("condition", format!("{:?}", escrow.condition)),
    ]))
}

fn execute_release_funds(
    ctx: ExecuteContext,
    recipient_addr: Option<String>,
    start_after: Option<String>,
    limit: Option<u32>,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    let recipient_addr = recipient_addr.unwrap_or_else(|| info.sender.to_string());

    let keys = get_keys_for_recipient(deps.storage, &recipient_addr, start_after, limit)?;

    ensure!(!keys.is_empty(), ContractError::NoLockedFunds {});

    let mut msgs: Vec<SubMsg> = vec![];
    for key in keys.iter() {
        let funds: Escrow = escrows().load(deps.storage, key.clone())?;
        if !funds.is_locked(&env.block)? {
            let msg = funds
                .recipient
                .generate_direct_msg(&deps.as_ref(), funds.coins)?;
            msgs.push(msg);
            escrows().remove(deps.storage, key.clone())?;
        }
    }

    ensure!(!msgs.is_empty(), ContractError::FundsAreLocked {});

    Ok(Response::new().add_submessages(msgs).add_attributes(vec![
        attr("action", "release_funds"),
        attr("recipient_addr", recipient_addr),
    ]))
}

fn execute_release_specific_funds(
    ctx: ExecuteContext,
    owner: String,
    recipient: Option<String>,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    let recipient = recipient.unwrap_or_else(|| info.sender.to_string());
    let key = get_key(&owner, &recipient);
    let escrow = escrows().may_load(deps.storage, key.clone())?;
    match escrow {
        None => Err(ContractError::NoLockedFunds {}),
        Some(escrow) => {
            ensure!(
                !escrow.is_locked(&env.block)?,
                ContractError::FundsAreLocked {}
            );
            escrows().remove(deps.storage, key)?;
            let msg = escrow
                .recipient
                .generate_direct_msg(&deps.as_ref(), escrow.coins)?;
            Ok(Response::new().add_submessage(msg).add_attributes(vec![
                attr("action", "release_funds"),
                attr("recipient_addr", recipient),
            ]))
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, env, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetLockedFunds { owner, recipient } => {
            encode_binary(&query_held_funds(deps, owner, recipient)?)
        }
        QueryMsg::GetLockedFundsForRecipient {
            recipient,
            start_after,
            limit,
        } => encode_binary(&query_funds_for_recipient(
            deps,
            recipient,
            start_after,
            limit,
        )?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

fn query_funds_for_recipient(
    deps: Deps,
    recipient: String,
    start_after: Option<String>,
    limit: Option<u32>,
) -> Result<GetLockedFundsForRecipientResponse, ContractError> {
    let keys = get_keys_for_recipient(deps.storage, &recipient, start_after, limit)?;
    let mut recipient_escrows: Vec<Escrow> = vec![];
    for key in keys.iter() {
        recipient_escrows.push(escrows().load(deps.storage, key.to_vec())?);
    }
    Ok(GetLockedFundsForRecipientResponse {
        funds: recipient_escrows,
    })
}

fn query_held_funds(
    deps: Deps,
    owner: String,
    recipient: String,
) -> Result<GetLockedFundsResponse, ContractError> {
    let hold_funds = escrows().may_load(deps.storage, get_key(&owner, &recipient))?;
    Ok(GetLockedFundsResponse { funds: hold_funds })
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
