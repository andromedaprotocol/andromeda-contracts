use crate::state::SPLITTER;
use andromeda_finance::splitter::{
    validate_expiry_duration, validate_recipient_list, AddressPercent, Cw20HookMsg, ExecuteMsg,
    GetSplitterConfigResponse, InstantiateMsg, QueryMsg, Splitter,
};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    amp::{messages::AMPPkt, Recipient},
    andr_execute_fn,
    common::{encode_binary, expiration::Expiry},
    error::ContractError,
};
use andromeda_std::{ado_contract::ADOContract, common::context::ExecuteContext};
use cosmwasm_std::{
    attr, coin, ensure, entry_point, from_json, Binary, Coin, Deps, DepsMut, Env, MessageInfo,
    Reply, Response, StdError, SubMsg, Uint128,
};
use cw20::{Cw20Coin, Cw20ReceiveMsg};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-splitter";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let splitter = Splitter {
        recipients: msg.recipients.clone(),
        lock: msg
            .clone()
            .lock_time
            .map(|lock_time| validate_expiry_duration(&lock_time, &env.block))
            .transpose()?
            .unwrap_or_default(),
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
    let res = match msg {
        ExecuteMsg::UpdateRecipients { recipients } => execute_update_recipients(ctx, recipients),
        ExecuteMsg::UpdateLock { lock_time } => execute_update_lock(ctx, lock_time),
        ExecuteMsg::UpdateDefaultRecipient { recipient } => {
            execute_update_default_recipient(ctx, recipient)
        }
        ExecuteMsg::Send { config } => execute_send(ctx, config),
        ExecuteMsg::Receive(receive_msg) => handle_receive_cw20(ctx, receive_msg),
        _ => ADOContract::default().execute(ctx, msg),
    }?;

    Ok(res)
}

pub fn handle_receive_cw20(
    ctx: ExecuteContext,
    receive_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let ExecuteContext { ref raw_info, .. } = ctx;
    let asset_sent = raw_info.sender.clone().into_string();
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

fn execute_send(
    ctx: ExecuteContext,
    config: Option<Vec<AddressPercent>>,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;
    ensure!(
        !info.funds.is_empty(),
        ContractError::InvalidFunds {
            msg: "At least one coin should to be sent".to_string(),
        }
    );
    for coin in info.funds.clone() {
        ensure!(
            !coin.amount.is_zero(),
            ContractError::InvalidFunds {
                msg: "Amount must be non-zero".to_string(),
            }
        );
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

    let mut remainder_funds = info.funds.clone();
    // Looking at this nested for loop, we could find a way to reduce time/memory complexity to avoid DoS.
    // Would like to understand more about why we loop through funds and what it exactly stored in it.
    // From there we could look into HashMaps, or other methods to break the nested loops and avoid Denial of Service.
    // [ACK-04] Limit number of coins sent to 5.
    ensure!(
        info.funds.len() < 5,
        ContractError::ExceedsMaxAllowedCoins {}
    );

    let mut pkt = AMPPkt::from_ctx(ctx.amp_ctx, ctx.env.contract.address.to_string());

    for recipient_addr in splitter_recipients {
        let recipient_percent = recipient_addr.percent;
        for (i, coin) in info.funds.clone().iter().enumerate() {
            let amount_owed = coin.amount.mul_floor(recipient_percent);
            if !amount_owed.is_zero() {
                let mut recip_coin: Coin = coin.clone();
                recip_coin.amount = amount_owed;
                remainder_funds[i].amount =
                    remainder_funds[i].amount.checked_sub(recip_coin.amount)?;
                amp_funds.push(recip_coin.clone());

                let amp_msg = recipient_addr
                    .recipient
                    .generate_amp_msg(&deps.as_ref(), Some(vec![recip_coin.clone()]))?;
                pkt = pkt.add_message(amp_msg);
            }
        }
    }
    remainder_funds.retain(|x| x.amount > Uint128::zero());

    // Why does the remaining funds go the the sender of the executor of the splitter?
    // Is it considered tax(fee) or mistake?
    // Discussion around caller of splitter function in andromedaSPLITTER smart contract.
    // From tests, it looks like owner of smart contract (Andromeda) will recieve the rest of funds.
    // If so, should be documented
    if !remainder_funds.is_empty() {
        let remainder_recipient = splitter
            .default_recipient
            .unwrap_or(Recipient::new(info.sender.to_string(), None));
        let native_msg =
            remainder_recipient.generate_direct_msg(&deps.as_ref(), remainder_funds)?;
        msgs.push(native_msg);
    }
    let kernel_address = ctx.contract.get_kernel_address(deps.as_ref().storage)?;

    if !pkt.messages.is_empty() {
        let distro_msg = pkt.to_sub_msg(kernel_address, Some(amp_funds), 1)?;
        msgs.push(distro_msg);
    }

    Ok(Response::new()
        .add_submessages(msgs)
        .add_attribute("action", "send")
        .add_attribute("sender", info.sender.to_string()))
}

fn execute_send_cw20(
    ctx: ExecuteContext,
    sender: String,
    amount: Uint128,
    asset: String,
    config: Option<Vec<AddressPercent>>,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, .. } = ctx;
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
    let mut remainder_funds = coin(amount.u128(), asset.clone());

    let mut pkt = AMPPkt::from_ctx(ctx.amp_ctx, ctx.env.contract.address.to_string());
    for recipient_addr in splitter_recipients {
        let recipient_percent = recipient_addr.percent;
        let mut vec_coin: Vec<Coin> = Vec::new();
        let coin = coin(amount.u128(), asset.clone());
        let amount_owed = coin.amount.mul_floor(recipient_percent);

        if !amount_owed.is_zero() {
            let mut recip_coin: Coin = coin.clone();
            recip_coin.amount = amount_owed;
            remainder_funds.amount = remainder_funds.amount.checked_sub(recip_coin.amount)?;
            vec_coin.push(recip_coin.clone());
            amp_funds.push(recip_coin.clone());
            let amp_msg = recipient_addr
                .recipient
                .generate_amp_msg(&deps.as_ref(), Some(vec_coin))?;
            pkt = pkt.add_message(amp_msg);
        }
    }

    if !remainder_funds.amount.is_zero() {
        let remainder_recipient = splitter
            .default_recipient
            .unwrap_or(Recipient::new(sender.clone(), None));
        let cw20_msg = remainder_recipient.generate_msg_cw20(
            &deps.as_ref(),
            Cw20Coin {
                address: asset,
                amount: remainder_funds.amount,
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
        .add_attribute("action", "cw20_send")
        .add_attribute("sender", sender.to_string()))
}

fn execute_update_recipients(
    ctx: ExecuteContext,
    recipients: Vec<AddressPercent>,
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

    let new_lock_time_expiration = validate_expiry_duration(&lock_time, &env.block)?;
    // Set new lock time
    splitter.lock = new_lock_time_expiration;

    SPLITTER.save(deps.storage, &splitter)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "update_lock"),
        attr("locked", new_lock_time_expiration.to_string()),
    ]))
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
