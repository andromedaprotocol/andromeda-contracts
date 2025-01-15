use crate::state::SPLITTER;
use andromeda_finance::{
    splitter::validate_expiry_duration,
    weighted_splitter::{
        AddressWeight, ExecuteMsg, GetSplitterConfigResponse, GetUserWeightResponse,
        InstantiateMsg, QueryMsg, Splitter,
    },
};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    ado_contract::ADOContract,
    amp::{AndrAddr, Recipient},
    common::{actions::call_action, context::ExecuteContext, encode_binary, expiration::Expiry},
    error::ContractError,
};
use cosmwasm_std::{
    attr, ensure, entry_point, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Reply, Response,
    StdError, SubMsg, Uint128,
};
use cw_utils::nonpayable;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-weighted-distribution-splitter";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // Max 100 recipients
    ensure!(
        msg.recipients.len() <= 100,
        ContractError::ReachedRecipientLimit {}
    );

    let lock = msg
        .lock_time
        .map(|lock_time| validate_expiry_duration(&lock_time, &env.block))
        .transpose()?
        .unwrap_or_default();

    let splitter = Splitter {
        recipients: msg.recipients,
        lock,
        default_recipient: msg.default_recipient,
    };

    SPLITTER.save(deps.storage, &splitter)?;
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
    call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;
    match msg {
        ExecuteMsg::UpdateRecipients { recipients } => execute_update_recipients(ctx, recipients),
        ExecuteMsg::UpdateRecipientWeight { recipient } => {
            execute_update_recipient_weight(ctx, recipient)
        }
        ExecuteMsg::AddRecipient { recipient } => execute_add_recipient(ctx, recipient),
        ExecuteMsg::RemoveRecipient { recipient } => execute_remove_recipient(ctx, recipient),
        ExecuteMsg::UpdateLock { lock_time } => execute_update_lock(ctx, lock_time),
        ExecuteMsg::UpdateDefaultRecipient { recipient } => {
            execute_update_default_recipient(ctx, recipient)
        }
        ExecuteMsg::Send { config } => execute_send(ctx, config),

        _ => ADOContract::default().execute(ctx, msg),
    }
}

pub fn execute_update_recipient_weight(
    ctx: ExecuteContext,
    recipient: AddressWeight,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    nonpayable(&info)?;
    // Only the contract's owner can update a recipient's weight
    ensure!(
        ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    // Can't set weight to 0
    ensure!(
        recipient.weight > Uint128::zero(),
        ContractError::InvalidWeight {}
    );

    // Splitter's lock should be expired
    let mut splitter = SPLITTER.load(deps.storage)?;

    ensure!(
        splitter.lock.is_expired(&env.block),
        ContractError::ContractLocked { msg: None }
    );

    // Recipients are stored in a vector, we search for the desired recipient's index in the vector

    let user_index = splitter
        .recipients
        .clone()
        .into_iter()
        .position(|x| x.recipient == recipient.recipient);

    // If the index exists, change the element's weight.
    // If the index doesn't exist, the recipient isn't on the list
    ensure!(user_index.is_some(), ContractError::UserNotFound {});

    if let Some(i) = user_index {
        splitter.recipients[i].weight = recipient.weight;
        SPLITTER.save(deps.storage, &splitter)?;
    };
    Ok(Response::default().add_attribute("action", "updated_recipient_weight"))
}

fn execute_update_default_recipient(
    ctx: ExecuteContext,
    recipient: Option<Recipient>,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    nonpayable(&info)?;

    ensure!(
        ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

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

pub fn execute_add_recipient(
    ctx: ExecuteContext,
    recipient: AddressWeight,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    nonpayable(&info)?;

    // Only the contract's owner can add a recipient
    ensure!(
        ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    // No need to send funds

    // Check if splitter is locked
    let mut splitter = SPLITTER.load(deps.storage)?;

    // Can't add recipients while the lock isn't expired

    ensure!(
        splitter.lock.is_expired(&env.block),
        ContractError::ContractLocked { msg: None }
    );

    // Can't set weight to 0
    ensure!(
        recipient.weight > Uint128::zero(),
        ContractError::InvalidWeight {}
    );

    // Check for duplicate recipients
    let user_exists = splitter.recipients.iter().any(|x| {
        x.recipient.address.get_raw_address(&deps.as_ref())
            == recipient.recipient.address.get_raw_address(&deps.as_ref())
    });

    ensure!(!user_exists, ContractError::DuplicateRecipient {});

    // Adding a recipient can't push the total number of recipients over 100

    ensure!(
        splitter.recipients.len() < 100,
        ContractError::ReachedRecipientLimit {}
    );

    splitter.recipients.push(recipient);
    let new_splitter = Splitter {
        recipients: splitter.recipients,
        lock: splitter.lock,
        default_recipient: splitter.default_recipient,
    };
    SPLITTER.save(deps.storage, &new_splitter)?;

    Ok(Response::default().add_attributes(vec![attr("action", "added_recipient")]))
}

fn execute_send(
    ctx: ExecuteContext,
    config: Option<Vec<AddressWeight>>,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;
    // Amount of coins sent should be at least 1
    ensure!(
        !&info.funds.is_empty(),
        ContractError::InvalidFunds {
            msg: "At least one coin should be sent".to_string(),
        }
    );
    // Can't send more than 5 types of coins
    ensure!(
        info.funds.len() < 5,
        ContractError::ExceedsMaxAllowedCoins {}
    );
    let splitter = SPLITTER.load(deps.storage)?;
    let splitter_recipients = if let Some(config) = config {
        ensure!(
            splitter.lock.is_expired(&ctx.env.block),
            ContractError::ContractLocked {
                msg: Some("Config isn't allowed while the splitter is locked".to_string())
            }
        );
        // Max 100 recipients
        ensure!(config.len() <= 100, ContractError::ReachedRecipientLimit {});
        config
    } else {
        splitter.recipients
    };
    let mut msgs: Vec<SubMsg> = Vec::new();
    let mut remainder_funds = info.funds.clone();
    let mut total_weight = Uint128::zero();

    // Calculate the total weight of all recipients
    for recipient_addr in &splitter_recipients {
        let recipient_weight = recipient_addr.weight;
        total_weight = total_weight.checked_add(recipient_weight)?;
    }

    // Each recipient recieves the funds * (the recipient's weight / total weight of all recipients)
    // The remaining funds go to the sender of the function
    for recipient_addr in &splitter_recipients {
        let recipient_weight = recipient_addr.weight;
        let mut vec_coin: Vec<Coin> = Vec::new();
        for (i, coin) in info.funds.iter().enumerate() {
            let mut recip_coin: Coin = coin.clone();
            recip_coin.amount = coin.amount.multiply_ratio(recipient_weight, total_weight);
            remainder_funds[i].amount = remainder_funds[i].amount.checked_sub(recip_coin.amount)?;
            vec_coin.push(recip_coin);
        }
        // ADO receivers must use AndromedaMsg::Receive to execute their functionality
        // Others may just receive the funds
        let direct_message = recipient_addr
            .recipient
            .generate_direct_msg(&deps.as_ref(), vec_coin)?;
        msgs.push(direct_message);
    }
    remainder_funds.retain(|x| x.amount > Uint128::zero());

    if !remainder_funds.is_empty() {
        let remainder_recipient = splitter
            .default_recipient
            .unwrap_or(Recipient::new(info.sender.to_string(), None));
        let native_msg =
            remainder_recipient.generate_direct_msg(&deps.as_ref(), remainder_funds)?;
        msgs.push(native_msg);
    }

    // // Generates the SubMsg intended for the kernel
    // // Check if any messages are intended for kernel in the first place
    // let contract = ADOContract::default();

    // // The original sender of the message
    // let origin = match packet {
    //     Some(p) => p.get_verified_origin(),
    //     None => info.sender.to_string(),
    // };

    // // The previous sender of the message is the contract
    // let previous_sender = env.contract.address;

    // if !amp_msgs.is_empty() {
    //     // The kernel address has been validated and saved during instantiation
    //     let kernel_address = contract.get_kernel_address(deps.storage)?;

    //     let msg = generate_msg_native_kernel(
    //         kernel_funds,
    //         origin,
    //         previous_sender.into_string(),
    //         amp_msgs,
    //         kernel_address.into_string(),
    //     )?;
    //     msgs.push(msg);
    // }

    Ok(Response::new()
        .add_submessages(msgs)
        .add_attributes(vec![attr("action", "send"), attr("sender", info.sender)]))
}

fn execute_update_recipients(
    ctx: ExecuteContext,
    recipients: Vec<AddressWeight>,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    nonpayable(&info)?;

    // Only the owner can use this function
    ensure!(
        ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    // No need to send funds

    // Recipient list can't be empty
    ensure!(
        !recipients.is_empty(),
        ContractError::EmptyRecipientsList {}
    );

    let mut splitter = SPLITTER.load(deps.storage)?;

    // Can't update recipients while lock isn't expired
    ensure!(
        splitter.lock.is_expired(&env.block),
        ContractError::ContractLocked { msg: None }
    );

    // Maximum number of recipients is 100
    ensure!(
        recipients.len() <= 100,
        ContractError::ReachedRecipientLimit {}
    );

    // A recipient's weight has to be greater than zero
    let zero_weight = recipients.iter().any(|x| x.weight == Uint128::zero());

    ensure!(!zero_weight, ContractError::InvalidWeight {});

    splitter.recipients = recipients;
    SPLITTER.save(deps.storage, &splitter)?;

    Ok(Response::default().add_attributes(vec![attr("action", "update_recipients")]))
}

fn execute_remove_recipient(
    ctx: ExecuteContext,
    recipient: AndrAddr,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    nonpayable(&info)?;

    ensure!(
        ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let mut splitter = SPLITTER.load(deps.storage)?;

    ensure!(
        splitter.lock.is_expired(&env.block),
        ContractError::ContractLocked { msg: None }
    );

    // Find and remove recipient in one pass
    let recipient_idx = splitter
        .recipients
        .iter()
        .position(|x| {
            x.recipient.address.get_raw_address(&deps.as_ref())
                == recipient.get_raw_address(&deps.as_ref())
        })
        .ok_or(ContractError::UserNotFound {})?;

    splitter.recipients.swap_remove(recipient_idx);
    SPLITTER.save(deps.storage, &splitter)?;

    Ok(Response::default().add_attributes(vec![attr("action", "removed_recipient")]))
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

    let mut splitter = SPLITTER.load(deps.storage)?;

    // Can't call this function while the lock isn't expired

    ensure!(
        splitter.lock.is_expired(&env.block),
        ContractError::ContractLocked { msg: None }
    );

    let new_lock_time_expiration = validate_expiry_duration(&lock_time, &env.block)?;

    splitter.lock = new_lock_time_expiration;

    SPLITTER.save(deps.storage, &splitter)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "update_lock"),
        attr("locked", new_lock_time_expiration.to_string()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, env, CONTRACT_NAME, CONTRACT_VERSION)
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetSplitterConfig {} => encode_binary(&query_splitter(deps)?),
        QueryMsg::GetUserWeight { user } => encode_binary(&query_user_weight(deps, user)?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

fn query_user_weight(deps: Deps, user: AndrAddr) -> Result<GetUserWeightResponse, ContractError> {
    let splitter = SPLITTER.load(deps.storage)?;
    let recipients = splitter.recipients;

    let addrs = recipients
        .iter()
        .find(|&x| x.recipient.address.get_raw_address(&deps) == user.get_raw_address(&deps))
        .ok_or(ContractError::AccountNotFound {})?;

    // Calculate the total weight
    let mut total_weight = Uint128::zero();
    for recipient_addr in &recipients {
        let recipient_weight = recipient_addr.weight;
        total_weight = total_weight.checked_add(recipient_weight)?;
    }
    Ok(GetUserWeightResponse {
        weight: addrs.weight,
        total_weight,
    })
}

fn query_splitter(deps: Deps) -> Result<GetSplitterConfigResponse, ContractError> {
    let splitter = SPLITTER.load(deps.storage)?;

    Ok(GetSplitterConfigResponse { config: splitter })
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
