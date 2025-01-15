use crate::state::{
    read_sale_infos, sale_infos, SaleInfo, TokenSaleState, NEXT_SALE_ID, TOKEN_SALE_STATE,
};
use std::vec;

use andromeda_non_fungible_tokens::marketplace::{
    Cw20HookMsg, Cw721HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, SaleIdsResponse,
    SaleStateResponse, Status,
};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    ado_contract::ADOContract,
    amp::Recipient,
    common::{
        actions::call_action,
        context::ExecuteContext,
        denom::{
            authorize_addresses, execute_authorize_contract, execute_deauthorize_contract, Asset,
            AuthorizedAddressesResponse, PermissionAction, SEND_CW20_ACTION, SEND_NFT_ACTION,
        },
        encode_binary,
        expiration::{expiration_from_milliseconds, get_and_validate_start_time, Expiry},
        rates::{get_tax_amount, get_tax_amount_cw20},
        Funds, Milliseconds, MillisecondsDuration, OrderBy,
    },
    error::ContractError,
};

use cw20::{Cw20Coin, Cw20ReceiveMsg};
use cw721::{Cw721ExecuteMsg, Cw721QueryMsg, Cw721ReceiveMsg, OwnerOfResponse};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, coin, ensure, from_json, Addr, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    QuerierWrapper, QueryRequest, Reply, Response, StdError, Storage, SubMsg, Uint128, WasmMsg,
    WasmQuery,
};

use cw_utils::{nonpayable, Expiration};

const CONTRACT_NAME: &str = "crates.io:andromeda-marketplace";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    NEXT_SALE_ID.save(deps.storage, &Uint128::from(1u128))?;
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

    if let Some(authorized_token_addresses) = msg.authorized_token_addresses {
        authorize_addresses(&mut deps, SEND_NFT_ACTION, authorized_token_addresses)?;
    }

    if let Some(authorized_cw20_addresses) = msg.authorized_cw20_addresses {
        authorize_addresses(&mut deps, SEND_CW20_ACTION, authorized_cw20_addresses)?;
    }

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
    let action = msg.as_ref().to_string();

    let action_response = call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;

    let res = match msg {
        ExecuteMsg::ReceiveNft(msg) => handle_receive_cw721(ctx, msg),
        ExecuteMsg::Receive(msg) => handle_receive_cw20(ctx, msg),
        ExecuteMsg::UpdateSale {
            token_id,
            token_address,
            coin_denom,
            price,
            recipient,
        } => execute_update_sale(ctx, token_id, token_address, price, coin_denom, recipient),
        ExecuteMsg::Buy {
            token_id,
            token_address,
        } => execute_buy(ctx, token_id, token_address, action),
        ExecuteMsg::CancelSale {
            token_id,
            token_address,
        } => execute_cancel(ctx, token_id, token_address),
        ExecuteMsg::AuthorizeContract {
            action,
            addr,
            expiration,
        } => execute_authorize_contract(ctx.deps, ctx.info, action, addr, expiration),

        ExecuteMsg::DeauthorizeContract { action, addr } => {
            execute_deauthorize_contract(ctx.deps, ctx.info, action, addr)
        }

        _ => ADOContract::default().execute(ctx, msg),
    }?;
    Ok(res
        .add_submessages(action_response.messages)
        .add_attributes(action_response.attributes)
        .add_events(action_response.events))
}

fn handle_receive_cw721(
    mut ctx: ExecuteContext,
    msg: Cw721ReceiveMsg,
) -> Result<Response, ContractError> {
    ADOContract::default().is_permissioned(
        ctx.deps.branch(),
        ctx.env.clone(),
        SEND_NFT_ACTION,
        ctx.info.sender.clone(),
    )?;

    match from_json(&msg.msg)? {
        Cw721HookMsg::StartSale {
            price,
            coin_denom,
            start_time,
            duration,
            recipient,
        } => execute_start_sale(
            ctx.deps,
            ctx.env,
            msg.sender,
            msg.token_id,
            ctx.info.sender.to_string(),
            price,
            start_time,
            coin_denom,
            duration,
            recipient,
        ),
    }
}

pub fn handle_receive_cw20(
    mut ctx: ExecuteContext,
    receive_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    ADOContract::default().is_permissioned(
        ctx.deps.branch(),
        ctx.env.clone(),
        SEND_CW20_ACTION,
        ctx.info.sender.clone(),
    )?;
    let ExecuteContext { ref info, .. } = ctx;
    nonpayable(info)?;

    let asset_sent = info.sender.clone();
    let amount_sent = receive_msg.amount;
    let sender = receive_msg.sender;

    ensure!(
        !amount_sent.is_zero(),
        ContractError::InvalidFunds {
            msg: "Cannot send a 0 amount".to_string()
        }
    );

    match from_json(&receive_msg.msg)? {
        Cw20HookMsg::Buy {
            token_id,
            token_address,
        } => execute_buy_cw20(
            ctx,
            token_id,
            token_address,
            amount_sent,
            asset_sent,
            &sender,
            "Buy".to_string(),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_start_sale(
    mut deps: DepsMut,
    env: Env,
    sender: String,
    token_id: String,
    token_address: String,
    price: Uint128,
    start_time: Option<Expiry>,
    coin_denom: Asset,
    duration: Option<MillisecondsDuration>,
    recipient: Option<Recipient>,
) -> Result<Response, ContractError> {
    let (coin_denom, uses_cw20) = coin_denom.get_verified_asset(deps.branch(), env.clone())?;

    // Price can't be zero
    ensure!(price > Uint128::zero(), ContractError::InvalidZeroAmount {});

    // If start time wasn't provided, it will be set as the current_time
    let (start_expiration, _current_time) = get_and_validate_start_time(&env, start_time.clone())?;

    let end_expiration = if let Some(duration) = duration {
        ensure!(!duration.is_zero(), ContractError::InvalidExpiration {});
        expiration_from_milliseconds(
            start_time
                // If start time isn't provided, it is set one second in advance from the current time
                .unwrap_or(Expiry::FromNow(Milliseconds::from_seconds(1)))
                .get_time(&env.block)
                .plus_milliseconds(duration),
        )?
    } else {
        // If no duration is provided, the exipration will be set as Never
        Expiration::Never {}
    };

    let sale_id = get_and_increment_next_sale_id(deps.storage, &token_id, &token_address)?;

    TOKEN_SALE_STATE.save(
        deps.storage,
        sale_id.u128(),
        &TokenSaleState {
            coin_denom: coin_denom.clone(),
            sale_id,
            owner: sender,
            token_id: token_id.clone(),
            token_address: token_address.clone(),
            price,
            status: Status::Open,
            start_time: start_expiration,
            end_time: end_expiration,
            uses_cw20,
            recipient,
        },
    )?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "start_sale"),
        attr("status", "Open"),
        attr("coin_denom", coin_denom),
        attr("price", price),
        attr("sale_id", sale_id.to_string()),
        attr("token_id", token_id),
        attr("token_address", token_address),
        attr("start_time", start_expiration.to_string()),
        attr("end_time", end_expiration.to_string()),
        attr("uses_cw20", uses_cw20.to_string()),
    ]))
}

#[allow(clippy::too_many_arguments)]
fn execute_update_sale(
    ctx: ExecuteContext,
    token_id: String,
    token_address: String,
    price: Uint128,
    coin_denom: Asset,
    recipient: Option<Recipient>,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        mut deps,
        env,
        info,
        ..
    } = ctx;
    let (coin_denom, uses_cw20) = coin_denom.get_verified_asset(deps.branch(), env)?;
    nonpayable(&info)?;

    let mut token_sale_state =
        get_existing_token_sale_state(deps.storage, &token_id, &token_address)?;

    // Only token owner is authorized to update the sale
    ensure!(
        info.sender == token_sale_state.owner,
        ContractError::Unauthorized {}
    );

    // New price can't be zero
    ensure!(price > Uint128::zero(), ContractError::InvalidZeroAmount {});

    token_sale_state.price = price;
    token_sale_state.coin_denom.clone_from(&coin_denom);
    token_sale_state.uses_cw20 = uses_cw20;
    token_sale_state.recipient = recipient;
    TOKEN_SALE_STATE.save(
        deps.storage,
        token_sale_state.sale_id.u128(),
        &token_sale_state,
    )?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "update_sale"),
        attr("coin_denom", coin_denom),
        attr("price", price),
        attr("uses_cw20", uses_cw20.to_string()),
        attr("sale_id", token_sale_state.sale_id.to_string()),
        attr("token_id", token_id),
        attr("token_address", token_address),
    ]))
}

fn execute_buy(
    ctx: ExecuteContext,
    token_id: String,
    token_address: String,
    action: String,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let mut token_sale_state =
        get_existing_token_sale_state(deps.storage, &token_id, &token_address)?;

    let key = token_sale_state.sale_id.u128();

    match token_sale_state.status {
        Status::Open => {
            // Make sure the end time isn't expired, if it is we'll return an error and change the Status to expired in case if it's set as Open or Pending
            ensure!(
                !token_sale_state.end_time.is_expired(&env.block),
                ContractError::SaleExpired {}
            );

            // If start time hasn't expired, it means that the sale hasn't started yet.
            ensure!(
                token_sale_state.start_time.is_expired(&env.block),
                ContractError::SaleNotOpen {}
            );
        }
        Status::Expired => return Err(ContractError::SaleExpired {}),
        Status::Executed => return Err(ContractError::SaleExecuted {}),
        Status::Cancelled => return Err(ContractError::SaleCancelled {}),
    }

    // The owner can't buy his own NFT
    ensure!(
        token_sale_state.owner != info.sender,
        ContractError::TokenOwnerCannotBuy {}
    );

    // Only one coin can be sent
    ensure!(
        info.funds.len() == 1,
        ContractError::InvalidFunds {
            msg: "One coin should be sent.".to_string(),
        }
    );

    let token_owner = query_owner_of(
        deps.querier,
        token_sale_state.token_address.clone(),
        token_id.clone(),
    )?
    .owner;
    ensure!(
        // If this is false then the token is no longer held by the contract so the token has been
        // claimed.
        token_owner == env.contract.address,
        ContractError::SaleAlreadyConducted {}
    );

    let coin_denom = token_sale_state.coin_denom.clone();
    ensure!(
        !token_sale_state.uses_cw20,
        ContractError::InvalidFunds {
            msg: "Native funds were sent to a sale that only accepts cw20".to_string()
        }
    );
    let payment: &Coin = &info.funds[0];

    // Make sure funds are equal to the price and in the correct denomination
    ensure!(
        payment.denom == coin_denom,
        ContractError::InvalidFunds {
            msg: format!("No {coin_denom} assets are provided to sale"),
        }
    );

    let price = token_sale_state.price;
    ensure!(
        payment.amount >= price,
        ContractError::InvalidFunds {
            msg: format!("The funds sent don't match the price {price}"),
        }
    );

    // Change sale status from Open to Executed
    token_sale_state.status = Status::Executed;

    TOKEN_SALE_STATE.save(deps.storage, key, &token_sale_state)?;

    // Calculate the funds to be received after tax
    let (after_tax_payment, tax_messages) = purchase_token(
        deps.as_ref(),
        &info,
        None,
        token_sale_state.clone(),
        action.clone(),
    )?;

    let mut resp = Response::new()
        // Send tax/royalty messages
        .add_submessages(tax_messages)
        // Send NFT to buyer.
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token_sale_state.token_address.clone(),
            msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                recipient: info.sender.to_string(),
                token_id: token_id.clone(),
            })?,
            funds: vec![],
        }))
        .add_attribute("action", "buy")
        .add_attribute("token_id", token_id)
        .add_attribute("token_contract", token_sale_state.token_address)
        .add_attribute("recipient", info.sender.to_string())
        .add_attribute("sale_id", token_sale_state.sale_id);
    // Marketplace recipient's funds
    let sale_recipient_funds = after_tax_payment.try_get_coin()?;

    // It could be zero if the royalties are 100% of the sale price
    if !sale_recipient_funds.amount.is_zero() {
        // Get sale recipient's address
        let recipient = token_sale_state
            .recipient
            .unwrap_or(Recipient::from_string(token_sale_state.owner));

        // Send payment to recipient
        resp = resp.add_submessage(
            recipient.generate_direct_msg(&deps.as_ref(), vec![sale_recipient_funds])?,
        )
    }
    Ok(resp)
}

fn execute_buy_cw20(
    ctx: ExecuteContext,
    token_id: String,
    token_address: String,
    amount_sent: Uint128,
    asset_sent: Addr,
    // The user who sent the cw20
    sender: &str,
    action: String,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        mut deps,
        info,
        env,
        ..
    } = ctx;

    let mut token_sale_state =
        get_existing_token_sale_state(deps.storage, &token_id, &token_address)?;

    let key = token_sale_state.sale_id.u128();

    match token_sale_state.status {
        Status::Open => {
            // Make sure the end time isn't expired, if it is we'll return an error and change the Status to expired in case if it's set as Open or Pending
            ensure!(
                !token_sale_state.end_time.is_expired(&env.block),
                ContractError::SaleExpired {}
            );

            // If start time hasn't expired, it means that the sale hasn't started yet.
            ensure!(
                token_sale_state.start_time.is_expired(&env.block),
                ContractError::SaleNotOpen {}
            );
        }
        Status::Expired => return Err(ContractError::SaleExpired {}),
        Status::Executed => return Err(ContractError::SaleExecuted {}),
        Status::Cancelled => return Err(ContractError::SaleCancelled {}),
    }

    // The owner can't buy his own NFT
    ensure!(
        token_sale_state.owner != sender,
        ContractError::TokenOwnerCannotBuy {}
    );

    let token_owner = query_owner_of(
        deps.querier,
        token_sale_state.token_address.clone(),
        token_id.clone(),
    )?
    .owner;
    ensure!(
        // If this is false then the token is no longer held by the contract so the token has been
        // claimed.
        token_owner == env.contract.address,
        ContractError::SaleAlreadyConducted {}
    );

    let is_cw20_sale = token_sale_state.uses_cw20;
    ensure!(
        is_cw20_sale,
        ContractError::InvalidFunds {
            msg: "CW20 funds were sent to a sale that only accepts native funds".to_string()
        }
    );

    let sale_currency = token_sale_state.coin_denom.clone();
    let valid_cw20_sale = ADOContract::default()
        .is_permissioned(deps.branch(), env, SEND_CW20_ACTION, sale_currency.clone())
        .is_ok();
    ensure!(
        valid_cw20_sale,
        ContractError::InvalidAsset {
            asset: asset_sent.to_string()
        }
    );

    let payment: &Coin = &coin(amount_sent.u128(), asset_sent.to_string());

    // Make sure funds are equal to the price and in the correct denomination
    ensure!(
        payment.denom == sale_currency,
        ContractError::InvalidFunds {
            msg: format!("No {sale_currency} assets are provided to sale"),
        }
    );

    // Change sale status from Open to Executed
    token_sale_state.status = Status::Executed;

    TOKEN_SALE_STATE.save(deps.storage, key, &token_sale_state)?;

    // Calculate the funds to be received after tax
    let (after_tax_payment, tax_messages) = purchase_token(
        deps.as_ref(),
        &info,
        Some(amount_sent),
        token_sale_state.clone(),
        action,
    )?;

    let mut resp: Response = Response::new()
        // Send tax/royalty messages
        .add_submessages(tax_messages)
        // Send NFT to buyer.
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token_sale_state.token_address.clone(),
            msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                recipient: sender.to_string(),
                token_id: token_id.clone(),
            })?,
            funds: vec![],
        }))
        .add_attribute("action", "buy")
        .add_attribute("token_id", token_id)
        .add_attribute("token_contract", token_sale_state.token_address)
        .add_attribute("recipient", sender.to_string())
        .add_attribute("sale_id", token_sale_state.sale_id);

    match after_tax_payment {
        Funds::Cw20(cw20_after_tax_payment) => {
            if !cw20_after_tax_payment.amount.is_zero() {
                // Get sale recipient's address
                let recipient = token_sale_state
                    .recipient
                    .unwrap_or(Recipient::from_string(token_sale_state.owner));
                // Send payment to recipient
                resp = resp.add_submessage(
                    recipient.generate_msg_cw20(&deps.as_ref(), cw20_after_tax_payment)?,
                );
            }
        }
        Funds::Native(_) => {}
    }

    Ok(resp)
}

fn execute_cancel(
    ctx: ExecuteContext,
    token_id: String,
    token_address: String,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;
    nonpayable(&info)?;

    let mut token_sale_state =
        get_existing_token_sale_state(deps.storage, &token_id, &token_address)?;

    ensure!(
        info.sender == token_sale_state.owner,
        ContractError::Unauthorized {}
    );

    // Sale needs to be open or pending to be cancelled
    ensure!(
        token_sale_state.status == Status::Open,
        ContractError::SaleNotOpen {}
    );

    let messages: Vec<CosmosMsg> = vec![CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_sale_state.token_address.clone(),
        msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
            recipient: info.sender.to_string(),
            token_id: token_id.clone(),
        })?,
        funds: vec![],
    })];

    token_sale_state.status = Status::Cancelled;
    TOKEN_SALE_STATE.save(
        deps.storage,
        token_sale_state.sale_id.u128(),
        &token_sale_state,
    )?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "cancel")
        .add_attribute("status", "Cancelled")
        .add_attribute("token_id", token_id)
        .add_attribute("token_contract", token_sale_state.token_address)
        .add_attribute("sale_id", token_sale_state.sale_id)
        .add_attribute("recipient", info.sender))
}

fn purchase_token(
    deps: Deps,
    info: &MessageInfo,
    amount_sent: Option<Uint128>,
    state: TokenSaleState,
    action: String,
) -> Result<(Funds, Vec<SubMsg>), ContractError> {
    // Handle cw20 case
    if let Some(amount_sent) = amount_sent {
        let total_cost = Cw20Coin {
            address: state.coin_denom.clone(),
            amount: state.price,
        };
        let rates_response = ADOContract::default().query_deducted_funds(
            deps,
            action.clone(),
            Funds::Cw20(total_cost),
        )?;
        match rates_response {
            Some(rates_response) => {
                let remaining_amount = rates_response.leftover_funds.try_get_cw20_coin()?;
                let tax_amount =
                    get_tax_amount_cw20(&rates_response.msgs, state.price, remaining_amount.amount);
                let total_required_payment = state.price.checked_add(tax_amount)?;

                // Check that enough funds were sent to cover the required payment
                ensure!(
                    amount_sent.eq(&total_required_payment),
                    ContractError::InvalidFunds {
                        msg: format!(
                            "Invalid funds provided, expected: {}, received: {}",
                            total_required_payment, amount_sent
                        )
                    }
                );
                let after_tax_payment = Cw20Coin {
                    address: state.clone().coin_denom,
                    amount: remaining_amount.amount,
                };
                Ok((Funds::Cw20(after_tax_payment), rates_response.msgs))
            }
            // No rates response means that there's no tax
            None => {
                let payment = Cw20Coin {
                    address: state.coin_denom,
                    amount: state.price,
                };

                Ok((Funds::Cw20(payment), vec![]))
            }
        }
        // Handle native funds case
    } else {
        let total_cost = Coin::new(state.price.u128(), state.coin_denom.clone());
        let rates_response = ADOContract::default().query_deducted_funds(
            deps,
            action,
            Funds::Native(total_cost.clone()),
        )?;
        match rates_response {
            Some(rates_response) => {
                let remaining_amount = rates_response.leftover_funds.try_get_coin()?;
                let tax_amount =
                    get_tax_amount(&rates_response.msgs, state.price, remaining_amount.amount);

                let total_required_payment = state.price.checked_add(tax_amount)?;

                // Check that enough funds were sent to cover the required payment
                let amount_sent = info.funds[0].amount.u128();
                ensure!(
                    amount_sent.eq(&total_required_payment.u128()),
                    ContractError::InvalidFunds {
                        msg: format!(
                            "Invalid funds provided, expected: {}, received: {}",
                            total_required_payment, amount_sent
                        )
                    }
                );

                let after_tax_payment = Coin {
                    denom: state.clone().coin_denom,
                    amount: remaining_amount.amount,
                };

                Ok((Funds::Native(after_tax_payment), rates_response.msgs))
            }
            None => {
                let payment = Coin {
                    denom: state.coin_denom,
                    amount: state.price,
                };
                Ok((Funds::Native(payment), vec![]))
            }
        }
    }
}

fn get_existing_token_sale_state(
    storage: &dyn Storage,
    token_id: &str,
    token_address: &str,
) -> Result<TokenSaleState, ContractError> {
    let key = token_id.to_owned() + token_address;
    let latest_sale_id: Uint128 = match sale_infos().may_load(storage, &key)? {
        None => return Err(ContractError::SaleDoesNotExist {}),
        Some(sale_info) => *sale_info.last().unwrap(),
    };
    let token_sale_state = TOKEN_SALE_STATE.load(storage, latest_sale_id.u128())?;

    Ok(token_sale_state)
}

fn get_and_increment_next_sale_id(
    storage: &mut dyn Storage,
    token_id: &str,
    token_address: &str,
) -> Result<Uint128, ContractError> {
    let next_sale_id = NEXT_SALE_ID.load(storage)?;
    let incremented_next_sale_id = next_sale_id.checked_add(Uint128::from(1u128))?;
    NEXT_SALE_ID.save(storage, &incremented_next_sale_id)?;

    let key = token_id.to_owned() + token_address;

    let mut sale_info = sale_infos().load(storage, &key).unwrap_or_default();
    sale_info.push(next_sale_id);
    if sale_info.token_address.is_empty() {
        token_address.clone_into(&mut sale_info.token_address);
        token_id.clone_into(&mut sale_info.token_id);
    }
    sale_infos().save(storage, &key, &sale_info)?;
    Ok(next_sale_id)
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::LatestSaleState {
            token_id,
            token_address,
        } => encode_binary(&query_latest_sale_state(deps, token_id, token_address)?),
        QueryMsg::SaleState { sale_id } => encode_binary(&query_sale_state(deps, sale_id)?),
        QueryMsg::SaleIds {
            token_id,
            token_address,
        } => encode_binary(&query_sale_ids(deps, token_id, token_address)?),
        QueryMsg::SaleInfosForAddress {
            token_address,
            start_after,
            limit,
        } => encode_binary(&query_sale_infos_for_address(
            deps,
            token_address,
            start_after,
            limit,
        )?),
        QueryMsg::AuthorizedAddresses {
            action,
            start_after,
            limit,
            order_by,
        } => encode_binary(&query_authorized_addresses(
            deps,
            action,
            start_after,
            limit,
            order_by,
        )?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

fn query_sale_ids(
    deps: Deps,
    token_id: String,
    token_address: String,
) -> Result<SaleIdsResponse, ContractError> {
    let key = token_id + &token_address;
    let sale_info = sale_infos().may_load(deps.storage, &key)?;
    if let Some(sale_info) = sale_info {
        return Ok(SaleIdsResponse {
            sale_ids: sale_info.sale_ids,
        });
    }
    Ok(SaleIdsResponse { sale_ids: vec![] })
}

pub fn query_sale_infos_for_address(
    deps: Deps,
    token_address: String,
    start_after: Option<String>,
    limit: Option<u64>,
) -> Result<Vec<SaleInfo>, ContractError> {
    read_sale_infos(deps.storage, token_address, start_after, limit)
}

fn query_latest_sale_state(
    deps: Deps,
    token_id: String,
    token_address: String,
) -> Result<SaleStateResponse, ContractError> {
    let token_sale_state_result =
        get_existing_token_sale_state(deps.storage, &token_id, &token_address);
    if let Ok(token_sale_state) = token_sale_state_result {
        return Ok(token_sale_state.into());
    }
    Err(ContractError::SaleDoesNotExist {})
}

fn query_sale_state(deps: Deps, sale_id: Uint128) -> Result<SaleStateResponse, ContractError> {
    let token_sale_state = TOKEN_SALE_STATE.load(deps.storage, sale_id.u128())?;
    Ok(token_sale_state.into())
}

fn query_owner_of(
    querier: QuerierWrapper,
    token_addr: String,
    token_id: String,
) -> Result<OwnerOfResponse, ContractError> {
    let res: OwnerOfResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: token_addr,
        msg: encode_binary(&Cw721QueryMsg::OwnerOf {
            token_id,
            include_expired: None,
        })?,
    }))?;

    Ok(res)
}

fn query_authorized_addresses(
    deps: Deps,
    action: PermissionAction,
    start_after: Option<String>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> Result<AuthorizedAddressesResponse, ContractError> {
    let addresses = ADOContract::default().query_permissioned_actors(
        deps,
        action.as_str(),
        start_after,
        limit,
        order_by,
    )?;
    Ok(AuthorizedAddressesResponse { addresses })
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
