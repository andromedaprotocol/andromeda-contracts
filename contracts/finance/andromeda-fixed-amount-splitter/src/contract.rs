use std::collections::HashSet;

use crate::state::SPLITTER;
use andromeda_finance::{
    fixed_amount_splitter::{
        validate_recipient_list, AddressAmount, Cw20HookMsg, ExecuteMsg, GetSplitterConfigResponse,
        InstantiateMsg, QueryMsg, Splitter,
    },
    splitter::validate_expiry_duration,
};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    amp::{messages::AMPPkt, Recipient},
    andr_execute_fn,
    common::{encode_binary, expiration::Expiry, Milliseconds},
    error::ContractError,
};
use andromeda_std::{ado_contract::ADOContract, common::context::ExecuteContext};
use cosmwasm_std::{
    attr, coin, coins, ensure, entry_point, from_json, Binary, Coin, Deps, DepsMut, Env,
    MessageInfo, Reply, Response, StdError, SubMsg, Uint128,
};
use cw20::{Cw20Coin, Cw20ReceiveMsg};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-fixed-amount-splitter";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
// 1 day in seconds
const ONE_DAY: u64 = 86_400;
// 1 year in seconds
const ONE_YEAR: u64 = 31_536_000;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let lock = if let Some(ref lock_time) = msg.lock_time {
        let lock_seconds = lock_time.get_time(&env.block).seconds();
        ensure!(lock_seconds >= ONE_DAY, ContractError::LockTimeTooShort {});
        ensure!(lock_seconds <= ONE_YEAR, ContractError::LockTimeTooLong {});
        lock_time.get_time(&env.block)
    } else {
        Milliseconds::default()
    };
    let splitter = Splitter {
        recipients: msg.recipients.clone(),
        lock,
        default_recipient: msg.default_recipient.clone(),
    };

    // Save kernel address after validating it
    SPLITTER.save(deps.storage, &splitter)?;

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

    msg.validate(deps.as_ref())?;

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

#[andr_execute_fn]
pub fn execute(ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateRecipients { recipients } => execute_update_recipients(ctx, recipients),
        ExecuteMsg::UpdateLock { lock_time } => execute_update_lock(ctx, lock_time),
        ExecuteMsg::UpdateDefaultRecipient { recipient } => {
            execute_update_default_recipient(ctx, recipient)
        }
        ExecuteMsg::Receive(receive_msg) => handle_receive_cw20(ctx, receive_msg),
        ExecuteMsg::Send { config } => execute_send(ctx, config),
        _ => ADOContract::default().execute(ctx, msg),
    }
}

pub fn handle_receive_cw20(
    ctx: ExecuteContext,
    receive_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let ExecuteContext { ref info, .. } = ctx;
    let asset_sent = info.sender.clone().into_string();
    let amount_sent = receive_msg.amount;
    let sender = receive_msg.sender;

    ensure!(
        !amount_sent.is_zero(),
        ContractError::InvalidFunds {
            msg: "Cannot send a 0 amount".to_string()
        }
    );

    match from_json(&receive_msg.msg)? {
        Cw20HookMsg::Send { config } => {
            execute_send_cw20(ctx, sender, amount_sent, asset_sent, config)
        }
    }
}

fn execute_send_cw20(
    ctx: ExecuteContext,
    sender: String,
    amount: Uint128,
    asset: String,
    config: Option<Vec<AddressAmount>>,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;

    let coin = coin(amount.u128(), asset.clone());

    let splitter = SPLITTER.load(deps.storage)?;

    let splitter_recipients = if let Some(config) = config {
        ensure!(
            splitter.lock.is_expired(&ctx.env.block),
            ContractError::ContractLocked {
                msg: Some("Config isn't allowed while the splitter is locked".to_string())
            }
        );
        validate_recipient_list(deps.as_ref(), config.clone())?;
        config
    } else {
        splitter.recipients
    };

    let mut msgs: Vec<SubMsg> = Vec::new();
    let mut amp_funds: Vec<Coin> = Vec::new();
    let mut remainder_funds = coin.amount;

    let mut pkt = AMPPkt::from_ctx(ctx.amp_ctx, ctx.env.contract.address.to_string());
    for recipient in splitter_recipients.clone() {
        // Find the recipient's corresponding denom for the current iteration of the sent funds
        let recipient_coin = recipient
            .coins
            .clone()
            .into_iter()
            .find(|coin| coin.denom == asset);

        if let Some(recipient_coin) = recipient_coin {
            // Deduct from total amount
            remainder_funds = remainder_funds
                .checked_sub(recipient_coin.amount)
                .map_err(|_| ContractError::InsufficientFunds {})?;

            let recipient_funds =
                cosmwasm_std::coin(recipient_coin.amount.u128(), recipient_coin.denom);

            amp_funds.push(recipient_funds.clone());

            let amp_msg = recipient
                .recipient
                .generate_amp_msg(&deps.as_ref(), Some(vec![recipient_funds.clone()]))?;
            pkt = pkt.add_message(amp_msg);
        }
    }

    if !remainder_funds.is_zero() {
        let remainder_recipient = splitter
            .default_recipient
            .unwrap_or(Recipient::new(sender, None));
        let cw20_msg = remainder_recipient.generate_msg_cw20(
            &deps.as_ref(),
            Cw20Coin {
                address: asset,
                amount: remainder_funds,
            },
        )?;
        msgs.push(cw20_msg);
    }

    let kernel_address = ADOContract::default().get_kernel_address(deps.as_ref().storage)?;
    if !pkt.messages.is_empty() && !amp_funds.is_empty() {
        let distro_msg = pkt.to_sub_msg_cw20(kernel_address, amp_funds.clone(), 1)?;
        msgs.push(distro_msg.clone());
    }

    Ok(Response::new()
        .add_submessages(msgs)
        .add_attribute("action", "send")
        .add_attribute("sender", info.sender.to_string()))
}

fn execute_update_default_recipient(
    ctx: ExecuteContext,
    recipient: Option<Recipient>,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, env, .. } = ctx;
    let mut splitter = SPLITTER.load(deps.storage)?;

    // Can't call this function while the lock isn't expired
    ensure!(
        splitter.lock.is_expired(&env.block),
        ContractError::ContractLocked { msg: None }
    );

    if let Some(ref recipient) = recipient {
        recipient.validate(&deps.as_ref())?;
    }
    splitter.default_recipient = recipient;

    SPLITTER.save(deps.storage, &splitter)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "update_default_recipient"),
        attr(
            "recipient",
            splitter
                .default_recipient
                .map_or("no default recipient".to_string(), |r| {
                    r.address.to_string()
                }),
        ),
    ]))
}

fn execute_send(
    ctx: ExecuteContext,
    config: Option<Vec<AddressAmount>>,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;

    ensure!(
        info.funds.len() == 1 || info.funds.len() == 2,
        ContractError::InvalidFunds {
            msg: "A minimim of 1 and a maximum of 2 coins are allowed".to_string(),
        }
    );

    // Check against zero amounts and duplicate denoms
    let mut denom_set = HashSet::new();
    for coin in info.funds.clone() {
        ensure!(
            !coin.amount.is_zero(),
            ContractError::InvalidFunds {
                msg: "Amount must be non-zero".to_string(),
            }
        );
        ensure!(
            !denom_set.contains(&coin.denom),
            ContractError::DuplicateCoinDenoms {}
        );
        denom_set.insert(coin.denom);
    }
    let splitter = SPLITTER.load(deps.storage)?;
    let splitter_recipients = if let Some(config) = config {
        ensure!(
            splitter.lock.is_expired(&ctx.env.block),
            ContractError::ContractLocked {
                msg: Some("Config isn't allowed while the splitter is locked".to_string())
            }
        );
        validate_recipient_list(deps.as_ref(), config.clone())?;
        config
    } else {
        splitter.recipients
    };

    let mut msgs: Vec<SubMsg> = Vec::new();
    let mut amp_funds: Vec<Coin> = Vec::new();

    let mut pkt = AMPPkt::from_ctx(ctx.amp_ctx, ctx.env.contract.address.to_string());

    // Iterate through the sent funds
    for coin in info.funds {
        let mut remainder_funds = coin.amount;
        let denom = coin.denom;

        for recipient in splitter_recipients.clone() {
            // Find the recipient's corresponding denom for the current iteration of the sent funds
            let recipient_coin = recipient
                .coins
                .clone()
                .into_iter()
                .find(|coin| coin.denom == denom);

            if let Some(recipient_coin) = recipient_coin {
                // Deduct from total amount
                remainder_funds = remainder_funds
                    .checked_sub(recipient_coin.amount)
                    .map_err(|_| ContractError::InsufficientFunds {})?;

                let recipient_funds =
                    cosmwasm_std::coin(recipient_coin.amount.u128(), recipient_coin.denom);

                let amp_msg = recipient
                    .recipient
                    .generate_amp_msg(&deps.as_ref(), Some(vec![recipient_funds.clone()]))?;

                pkt = pkt.add_message(amp_msg);

                amp_funds.push(recipient_funds);
            }
        }

        // Refund message for sender
        if !remainder_funds.is_zero() {
            let remainder_recipient = splitter
                .default_recipient
                .clone()
                .unwrap_or(Recipient::new(info.sender.to_string(), None));
            let native_msg = remainder_recipient
                .generate_direct_msg(&deps.as_ref(), coins(remainder_funds.u128(), denom))?;
            msgs.push(native_msg);
        }
    }

    let kernel_address = ADOContract::default().get_kernel_address(deps.as_ref().storage)?;

    if !pkt.messages.is_empty() {
        let distro_msg = pkt.to_sub_msg(kernel_address, Some(amp_funds), 1)?;
        msgs.push(distro_msg);
    }

    Ok(Response::new()
        .add_submessages(msgs)
        .add_attribute("action", "send")
        .add_attribute("sender", info.sender.to_string()))
}

fn execute_update_recipients(
    ctx: ExecuteContext,
    recipients: Vec<AddressAmount>,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, env, .. } = ctx;

    validate_recipient_list(deps.as_ref(), recipients.clone())?;

    let mut splitter = SPLITTER.load(deps.storage)?;
    // Can't call this function while the lock isn't expired

    ensure!(
        splitter.lock.is_expired(&env.block),
        ContractError::ContractLocked { msg: None }
    );
    // Max 100 recipients
    ensure!(
        recipients.len() <= 100,
        ContractError::ReachedRecipientLimit {}
    );

    splitter.recipients = recipients;
    SPLITTER.save(deps.storage, &splitter)?;

    Ok(Response::default().add_attributes(vec![attr("action", "update_recipients")]))
}

fn execute_update_lock(ctx: ExecuteContext, lock_time: Expiry) -> Result<Response, ContractError> {
    let ExecuteContext { deps, env, .. } = ctx;

    let mut splitter = SPLITTER.load(deps.storage)?;

    // Can't call this function while the lock isn't expired
    ensure!(
        splitter.lock.is_expired(&env.block),
        ContractError::ContractLocked { msg: None }
    );

    let new_expiration = validate_expiry_duration(&lock_time, &env.block)?;

    splitter.lock = new_expiration;

    SPLITTER.save(deps.storage, &splitter)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "update_lock"),
        attr("locked", new_expiration.to_string()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, env, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetSplitterConfig {} => encode_binary(&query_splitter(deps)?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

fn query_splitter(deps: Deps) -> Result<GetSplitterConfigResponse, ContractError> {
    let splitter = SPLITTER.load(deps.storage)?;

    Ok(GetSplitterConfigResponse { config: splitter })
}
