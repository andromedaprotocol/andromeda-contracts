use crate::state::{
    auction_infos, read_auction_infos, read_bids, BIDS, NEXT_AUCTION_ID, TOKEN_AUCTION_STATE,
};
use andromeda_non_fungible_tokens::auction::{
    validate_auction, AuctionIdsResponse, AuctionInfo, AuctionStateResponse, Bid, BidsResponse,
    Cw20HookMsg, Cw721HookMsg, ExecuteMsg, InstantiateMsg, IsCancelledResponse, IsClaimedResponse,
    IsClosedResponse, QueryMsg, TokenAuctionState,
};
use andromeda_std::{
    ado_base::{
        permissioning::{LocalPermission, Permission},
        InstantiateMsg as BaseInstantiateMsg, MigrateMsg,
    },
    amp::Recipient,
    common::{
        actions::call_action,
        denom::{
            authorize_addresses, execute_authorize_contract, execute_deauthorize_contract,
            validate_native_denom, Asset, AuthorizedAddressesResponse, PermissionAction,
            SEND_CW20_ACTION,
        },
        encode_binary,
        expiration::{expiration_from_milliseconds, get_and_validate_start_time, Expiry},
        Funds, Milliseconds, OrderBy,
    },
    error::ContractError,
};
use andromeda_std::{ado_contract::ADOContract, common::context::ExecuteContext};

use cosmwasm_std::{
    attr, coins, ensure, entry_point, from_json, wasm_execute, Addr, BankMsg, Binary, Coin,
    CosmosMsg, Deps, DepsMut, Env, MessageInfo, QuerierWrapper, QueryRequest, Reply, Response,
    StdError, Storage, SubMsg, Uint128, WasmMsg, WasmQuery,
};
use cw20::{Cw20Coin, Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw721::{Cw721ExecuteMsg, Cw721QueryMsg, Cw721ReceiveMsg, OwnerOfResponse};
use cw_utils::nonpayable;

const CONTRACT_NAME: &str = "crates.io:andromeda-auction";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const SEND_NFT_ACTION: &str = "SEND_NFT";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    NEXT_AUCTION_ID.save(deps.storage, &Uint128::from(1u128))?;
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

    if let Some(authorized_token_addresses) = msg.authorized_token_addresses {
        authorize_addresses(&mut deps, SEND_NFT_ACTION, authorized_token_addresses)?;
    }

    if let Some(authorized_cw20_addresses) = msg.authorized_cw20_addresses {
        authorize_addresses(&mut deps, SEND_CW20_ACTION, authorized_cw20_addresses)?;
    }

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
        ExecuteMsg::UpdateAuction {
            token_id,
            token_address,
            start_time,
            end_time,
            coin_denom,
            whitelist,
            min_bid,
            min_raise,
            buy_now_price,
            recipient,
        } => execute_update_auction(
            ctx,
            token_id,
            token_address,
            start_time,
            end_time,
            coin_denom,
            whitelist,
            min_bid,
            min_raise,
            buy_now_price,
            recipient,
        ),
        ExecuteMsg::PlaceBid {
            token_id,
            token_address,
        } => execute_place_bid(ctx, token_id, token_address),
        ExecuteMsg::BuyNow {
            token_id,
            token_address,
        } => execute_buy_now(ctx, token_id, token_address, action),
        ExecuteMsg::CancelAuction {
            token_id,
            token_address,
        } => execute_cancel(ctx, token_id, token_address),
        ExecuteMsg::Claim {
            token_id,
            token_address,
        } => execute_claim(ctx, token_id, token_address, action),
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
        Cw721HookMsg::StartAuction {
            start_time,
            end_time,
            coin_denom,
            buy_now_price,
            whitelist,
            min_bid,
            min_raise,
            recipient,
        } => execute_start_auction(
            ctx,
            msg.sender,
            msg.token_id,
            start_time,
            end_time,
            coin_denom,
            buy_now_price,
            whitelist,
            min_bid,
            min_raise,
            recipient,
        ),
    }
}

pub fn handle_receive_cw20(
    mut ctx: ExecuteContext,
    receive_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let is_valid_cw20 = ADOContract::default()
        .is_permissioned(
            ctx.deps.branch(),
            ctx.env.clone(),
            SEND_CW20_ACTION,
            ctx.info.sender.clone(),
        )
        .is_ok();

    ensure!(
        is_valid_cw20,
        ContractError::InvalidAsset {
            asset: ctx.info.sender.into_string()
        }
    );

    let ExecuteContext { ref info, .. } = ctx;
    nonpayable(info)?;

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
        Cw20HookMsg::PlaceBid {
            token_id,
            token_address,
        } => execute_place_bid_cw20(
            ctx,
            token_id,
            token_address,
            amount_sent,
            asset_sent,
            &sender,
        ),
        Cw20HookMsg::BuyNow {
            token_id,
            token_address,
        } => execute_buy_now_cw20(
            ctx,
            token_id,
            token_address,
            "BuyNow".to_string(),
            amount_sent,
            asset_sent,
            &sender,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_start_auction(
    ctx: ExecuteContext,
    sender: String,
    token_id: String,
    start_time: Option<Expiry>,
    end_time: Expiry,
    coin_denom: Asset,
    buy_now_price: Option<Uint128>,
    whitelist: Option<Vec<Addr>>,
    min_bid: Option<Uint128>,
    min_raise: Option<Uint128>,
    recipient: Option<Recipient>,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        mut deps,
        info,
        env,
        ..
    } = ctx;
    let (coin_denom, uses_cw20) = coin_denom.get_verified_asset(deps.branch(), env.clone())?;
    ensure!(
        !end_time.get_time(&env.block).is_zero(),
        ContractError::InvalidExpiration {}
    );

    if let (Some(buy_now), Some(min)) = (buy_now_price, min_bid) {
        if min >= buy_now {
            return Err(ContractError::InvalidMinBid {
                msg: Some("buy_now_price must be greater than the min_bid".to_string()),
            });
        }
    }

    // If start time wasn't provided, it will be set as the current_time
    let (start_expiration, _current_time) = get_and_validate_start_time(&env, start_time)?;
    let end_expiration = expiration_from_milliseconds(end_time.get_time(&env.block))?;

    ensure!(
        end_expiration > start_expiration,
        ContractError::StartTimeAfterEndTime {}
    );

    let token_address = info.sender.to_string();

    let auction_id = get_and_increment_next_auction_id(deps.storage, &token_id, &token_address)?;
    BIDS.save(deps.storage, auction_id.u128(), &vec![])?;

    if let Some(ref whitelist) = whitelist {
        ADOContract::default().permission_action(auction_id.to_string(), deps.storage)?;

        for whitelisted_address in whitelist {
            ADOContract::set_permission(
                deps.storage,
                auction_id.to_string(),
                whitelisted_address,
                Permission::Local(LocalPermission::whitelisted(None, None)),
            )?;
        }
    };

    let whitelist_str = format!("{:?}", &whitelist);

    TOKEN_AUCTION_STATE.save(
        deps.storage,
        auction_id.u128(),
        &TokenAuctionState {
            start_time: start_expiration,
            end_time: end_expiration,
            high_bidder_addr: Addr::unchecked(""),
            high_bidder_amount: Uint128::zero(),
            buy_now_price,
            coin_denom: coin_denom.clone(),
            uses_cw20,
            auction_id,
            whitelist,
            min_bid,
            min_raise,
            owner: sender,
            token_id,
            token_address,
            is_cancelled: false,
            is_bought: false,
            recipient,
        },
    )?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "start_auction"),
        attr("start_time", start_expiration.to_string()),
        attr("end_time", end_expiration.to_string()),
        attr("coin_denom", coin_denom),
        attr("auction_id", auction_id.to_string()),
        attr("whitelist", whitelist_str),
    ]))
}

#[allow(clippy::too_many_arguments)]
fn execute_update_auction(
    ctx: ExecuteContext,
    token_id: String,
    token_address: String,
    start_time: Option<Expiry>,
    end_time: Expiry,
    coin_denom: Asset,
    whitelist: Option<Vec<Addr>>,
    min_bid: Option<Uint128>,
    min_raise: Option<Uint128>,
    buy_now_price: Option<Uint128>,
    recipient: Option<Recipient>,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        mut deps,
        info,
        env,
        ..
    } = ctx;
    nonpayable(&info)?;
    let (coin_denom, uses_cw20) = coin_denom.get_verified_asset(deps.branch(), env.clone())?;

    if uses_cw20 {
        let valid_cw20_auction = ADOContract::default()
            .is_permissioned(
                deps.branch(),
                env.clone(),
                SEND_CW20_ACTION,
                coin_denom.clone(),
            )
            .is_ok();
        ensure!(
            valid_cw20_auction,
            ContractError::InvalidFunds {
                msg: "Non-permissioned CW20 asset sent".to_string()
            }
        );
    } else {
        validate_native_denom(deps.as_ref(), coin_denom.clone())?;
    }
    let mut token_auction_state =
        get_existing_token_auction_state(deps.storage, &token_id, &token_address)?;
    ensure!(
        info.sender == token_auction_state.owner,
        ContractError::Unauthorized {}
    );
    ensure!(
        !token_auction_state.start_time.is_expired(&env.block),
        ContractError::AuctionAlreadyStarted {}
    );

    ensure!(
        !end_time.get_time(&env.block).is_zero(),
        ContractError::InvalidExpiration {}
    );

    // If start time wasn't provided, it will be set as the current_time
    let (start_expiration, _current_time) = get_and_validate_start_time(&env, start_time)?;
    let end_expiration = expiration_from_milliseconds(end_time.get_time(&env.block))?;

    ensure!(
        end_expiration > start_expiration,
        ContractError::StartTimeAfterEndTime {}
    );

    if let (Some(buy_now), Some(min)) = (buy_now_price, min_bid) {
        if min >= buy_now {
            return Err(ContractError::InvalidMinBid {
                msg: Some("buy_now_price must be greater than the min_bid".to_string()),
            });
        }
    }

    if let Some(ref whitelist) = whitelist {
        ADOContract::default()
            .permission_action(token_auction_state.auction_id.to_string(), deps.storage)?;

        for whitelisted_address in whitelist {
            ADOContract::set_permission(
                deps.storage,
                token_auction_state.auction_id.to_string(),
                whitelisted_address,
                Permission::Local(LocalPermission::whitelisted(None, None)),
            )?;
        }
    };

    let whitelist_str = format!("{:?}", &whitelist);

    token_auction_state.start_time = start_expiration;
    token_auction_state.end_time = end_expiration;
    token_auction_state.coin_denom.clone_from(&coin_denom);
    token_auction_state.uses_cw20 = uses_cw20;
    token_auction_state.min_bid = min_bid;
    token_auction_state.min_raise = min_raise;
    token_auction_state.buy_now_price = buy_now_price;
    token_auction_state.whitelist = whitelist;
    token_auction_state.recipient = recipient;
    TOKEN_AUCTION_STATE.save(
        deps.storage,
        token_auction_state.auction_id.u128(),
        &token_auction_state,
    )?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "update_auction"),
        attr("start_time", start_expiration.to_string()),
        attr("end_time", end_expiration.to_string()),
        attr("coin_denom", coin_denom),
        attr("uses_cw20", uses_cw20.to_string()),
        attr("auction_id", token_auction_state.auction_id.to_string()),
        attr("whitelist", format!("{:?}", whitelist_str)),
        attr("min_bid", format!("{:?}", &min_bid)),
        attr("min_raise", format!("{:?}", &min_raise)),
        attr("buy_now_price", format!("{:?}", &buy_now_price)),
    ]))
}

fn execute_place_bid(
    ctx: ExecuteContext,
    token_id: String,
    token_address: String,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        mut deps,
        info,
        env,
        ..
    } = ctx;
    let mut token_auction_state =
        get_existing_token_auction_state(deps.storage, &token_id, &token_address)?;

    ADOContract::default().is_permissioned(
        deps.branch(),
        env.clone(),
        token_auction_state.auction_id,
        info.sender.clone(),
    )?;

    validate_auction(token_auction_state.clone(), info.clone(), &env.block)?;

    ensure!(
        token_auction_state.high_bidder_addr != info.sender,
        ContractError::HighestBidderCannotOutBid {}
    );

    ensure!(
        !token_auction_state.uses_cw20,
        ContractError::InvalidFunds {
            msg: "Native funds were sent to an auction that only accepts cw20".to_string()
        }
    );

    let payment: &Coin = &info.funds[0];
    ensure!(
        payment.denom == token_auction_state.coin_denom,
        ContractError::InvalidFunds {
            msg: format!(
                "Invalid denomination: expected {}, got {}",
                token_auction_state.coin_denom, payment.denom
            ),
        }
    );
    ensure!(
        payment.amount.gt(&Uint128::zero()),
        ContractError::InvalidFunds {
            msg: "Amount of funds should be greater than 0".to_string(),
        }
    );
    let min_bid = token_auction_state.min_bid.unwrap_or(Uint128::zero());
    ensure!(
        payment.amount >= min_bid,
        ContractError::InvalidFunds {
            msg: format!(
                "Must provide at least {min_bid} {} to bid",
                token_auction_state.coin_denom
            )
        }
    );
    ensure!(
        token_auction_state.high_bidder_amount < payment.amount,
        ContractError::BidSmallerThanHighestBid {}
    );

    // If there's a min_raise, the difference between the new bid and the highest bid should be greater or equal to it.
    let min_raise = token_auction_state.min_raise.unwrap_or_default();
    let bid_difference = payment
        .amount
        .checked_sub(token_auction_state.high_bidder_amount)?;
    ensure!(
        bid_difference.ge(&min_raise),
        ContractError::MinRaiseUnmet {}
    );

    let mut messages: Vec<CosmosMsg> = vec![];
    // Send back previous bid unless there was no previous bid.
    if token_auction_state.high_bidder_amount > Uint128::zero() {
        let bank_msg = BankMsg::Send {
            to_address: token_auction_state.high_bidder_addr.to_string(),
            amount: coins(
                token_auction_state.high_bidder_amount.u128(),
                token_auction_state.coin_denom.clone(),
            ),
        };
        messages.push(CosmosMsg::Bank(bank_msg));
    }

    token_auction_state.high_bidder_addr = info.sender.clone();
    token_auction_state.high_bidder_amount = payment.amount;

    let key = token_auction_state.auction_id.u128();
    TOKEN_AUCTION_STATE.save(deps.storage, key, &token_auction_state)?;
    let mut bids_for_auction = BIDS.load(deps.storage, key)?;
    bids_for_auction.push(Bid {
        bidder: info.sender.to_string(),
        amount: payment.amount,
        timestamp: Milliseconds::from_nanos(env.block.time.nanos()),
    });
    BIDS.save(deps.storage, key, &bids_for_auction)?;
    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "bid"),
        attr("token_id", token_id),
        attr("bidder", info.sender.to_string()),
        attr("amount", payment.amount.to_string()),
    ]))
}

fn execute_buy_now(
    ctx: ExecuteContext,
    token_id: String,
    token_address: String,
    action: String,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        mut deps,
        info,
        env,
        ..
    } = ctx;
    let mut token_auction_state =
        get_existing_token_auction_state(deps.storage, &token_id, &token_address)?;

    // Make sure the auction has a Buy Now option
    let buy_now_price = token_auction_state
        .buy_now_price
        .map_or_else(|| Err(ContractError::NoBuyNowOption {}), Ok)?;

    ADOContract::default().is_permissioned(
        deps.branch(),
        env.clone(),
        token_auction_state.auction_id,
        info.sender.clone(),
    )?;

    validate_auction(token_auction_state.clone(), info.clone(), &env.block)?;

    ensure!(
        !token_auction_state.uses_cw20,
        ContractError::InvalidFunds {
            msg: "Native funds were sent to an auction that only accepts cw20".to_string()
        }
    );

    let payment: &Coin = &info.funds[0];

    ensure!(
        payment.denom == token_auction_state.coin_denom,
        ContractError::InvalidFunds {
            msg: format!(
                "Invalid denomination: expected {}, got {}",
                token_auction_state.coin_denom, payment.denom
            ),
        }
    );
    ensure!(
        payment.amount == buy_now_price,
        ContractError::InvalidFunds {
            msg: format!(
                "Incorrect amount: expected {}, got {}",
                buy_now_price, payment.amount
            ),
        }
    );

    let mut messages: Vec<CosmosMsg> = vec![];
    // Send back previous bid unless there was no previous bid.
    if token_auction_state.high_bidder_amount > Uint128::zero() {
        let bank_msg = BankMsg::Send {
            to_address: token_auction_state.high_bidder_addr.to_string(),
            amount: coins(
                token_auction_state.high_bidder_amount.u128(),
                token_auction_state.coin_denom.clone(),
            ),
        };
        messages.push(CosmosMsg::Bank(bank_msg));
    }

    token_auction_state.high_bidder_addr = info.sender.clone();
    token_auction_state.high_bidder_amount = payment.amount;
    // Set auction as bought
    token_auction_state.is_bought = true;

    let key = token_auction_state.auction_id.u128();
    TOKEN_AUCTION_STATE.save(deps.storage, key, &token_auction_state)?;

    // Calculate the funds to be received after tax
    let (after_tax_payment, tax_messages) = purchase_token(
        deps.as_ref(),
        &info,
        token_auction_state.clone(),
        action,
        payment.amount,
    )?;

    let mut resp: Response = Response::new()
        // Send NFT to auction winner.
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token_auction_state.token_address.clone(),
            msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                recipient: token_auction_state.high_bidder_addr.to_string(),
                token_id: token_id.clone(),
            })?,
            funds: vec![],
        }))
        // Refund to highest bidder
        .add_messages(messages)
        // Send tax/royalty messages
        .add_submessages(tax_messages)
        .add_attribute("action", "buy_now")
        .add_attribute("token_id", token_id.clone())
        .add_attribute("token_contract", token_auction_state.clone().token_address)
        .add_attribute("recipient", &token_auction_state.high_bidder_addr)
        .add_attribute("bought_at", token_auction_state.buy_now_price.unwrap())
        .add_attribute("auction_id", token_auction_state.auction_id);

    let recipient = token_auction_state
        .recipient
        .unwrap_or(Recipient::from_string(token_auction_state.owner));

    match after_tax_payment {
        Funds::Native(native_funds) => {
            // Send payment to recipient
            resp = resp
                .add_submessage(recipient.generate_direct_msg(&deps.as_ref(), vec![native_funds])?)
        }
        Funds::Cw20(cw20_funds) => {
            let cw20_msg = recipient.generate_msg_cw20(&deps.as_ref(), cw20_funds)?;
            resp = resp.add_submessage(cw20_msg)
        }
    }
    Ok(resp)
}

fn execute_place_bid_cw20(
    ctx: ExecuteContext,
    token_id: String,
    token_address: String,
    amount_sent: Uint128,
    asset_sent: String,
    // The user who sent the cw20
    sender: &str,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        mut deps,
        env,
        info,
        ..
    } = ctx;
    let mut token_auction_state =
        get_existing_token_auction_state(deps.storage, &token_id, &token_address)?;

    ADOContract::default().is_permissioned(
        deps.branch(),
        env.clone(),
        token_auction_state.auction_id,
        sender,
    )?;

    validate_auction(token_auction_state.clone(), info.clone(), &env.block)?;

    let sender_addr = deps.api.addr_validate(sender)?;

    ensure!(
        token_auction_state.high_bidder_addr != sender_addr,
        ContractError::HighestBidderCannotOutBid {}
    );

    let auction_currency = token_auction_state.clone().coin_denom;
    ensure!(
        auction_currency == asset_sent,
        ContractError::InvalidAsset { asset: asset_sent }
    );

    ensure!(
        token_auction_state.uses_cw20,
        ContractError::InvalidFunds {
            msg: "CW20 funds were sent to an auction that only accepts native funds".to_string()
        }
    );

    ensure!(
        amount_sent > Uint128::zero(),
        ContractError::InvalidFunds {
            msg: format!(
                "No {} assets are provided to auction",
                token_auction_state.coin_denom
            ),
        }
    );
    let min_bid = token_auction_state.min_bid.unwrap_or(Uint128::zero());
    ensure!(
        amount_sent >= min_bid,
        ContractError::InvalidFunds {
            msg: format!(
                "Must provide at least {min_bid} {} to bid",
                token_auction_state.coin_denom
            )
        }
    );
    ensure!(
        token_auction_state.high_bidder_amount < amount_sent,
        ContractError::BidSmallerThanHighestBid {}
    );

    // If there's a min_raise, the difference between the new bid and the highest bid should be greater or equal to it.
    let min_raise = token_auction_state.min_raise.unwrap_or_default();
    let bid_difference = amount_sent.checked_sub(token_auction_state.high_bidder_amount)?;
    ensure!(
        bid_difference.ge(&min_raise),
        ContractError::MinRaiseUnmet {}
    );

    let mut cw20_transfer: Vec<WasmMsg> = vec![];
    // Send back previous bid unless there was no previous bid.
    if token_auction_state.high_bidder_amount > Uint128::zero() {
        let transfer_msg = Cw20ExecuteMsg::Transfer {
            recipient: token_auction_state.high_bidder_addr.to_string(),
            amount: token_auction_state.high_bidder_amount,
        };
        let wasm_msg = wasm_execute(auction_currency, &transfer_msg, vec![])?;
        cw20_transfer.push(wasm_msg);
    }

    token_auction_state.high_bidder_addr = sender_addr.clone();
    token_auction_state.high_bidder_amount = amount_sent;

    let key = token_auction_state.auction_id.u128();
    TOKEN_AUCTION_STATE.save(deps.storage, key, &token_auction_state)?;
    let mut bids_for_auction = BIDS.load(deps.storage, key)?;
    bids_for_auction.push(Bid {
        bidder: sender.to_string(),
        amount: amount_sent,
        timestamp: Milliseconds::from_nanos(env.block.time.nanos()),
    });
    BIDS.save(deps.storage, key, &bids_for_auction)?;
    Ok(Response::new()
        .add_messages(cw20_transfer)
        .add_attributes(vec![
            attr("action", "bid"),
            attr("token_id", token_id),
            attr("bidder", sender_addr.to_string()),
            attr("amount", amount_sent.to_string()),
        ]))
}

fn execute_buy_now_cw20(
    ctx: ExecuteContext,
    token_id: String,
    token_address: String,
    action: String,
    amount_sent: Uint128,
    asset_sent: String,
    // The user who sent the cw20
    sender: &str,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        mut deps,
        info,
        env,
        ..
    } = ctx;
    let mut token_auction_state =
        get_existing_token_auction_state(deps.storage, &token_id, &token_address)?;

    // Make sure the auction has a Buy Now option
    let buy_now_price = token_auction_state
        .buy_now_price
        .map_or_else(|| Err(ContractError::NoBuyNowOption {}), Ok)?;

    validate_auction(token_auction_state.clone(), info.clone(), &env.block)?;

    ADOContract::default().is_permissioned(
        deps.branch(),
        env.clone(),
        token_auction_state.auction_id,
        info.sender.clone(),
    )?;

    let sender_addr = deps.api.addr_validate(sender)?;

    ensure!(
        token_auction_state.uses_cw20,
        ContractError::InvalidFunds {
            msg: "Native funds were sent to an auction that only accepts cw20".to_string()
        }
    );
    let auction_currency = token_auction_state.clone().coin_denom;

    ensure!(
        asset_sent == auction_currency,
        ContractError::InvalidFunds {
            msg: format!(
                "Invalid denomination: expected {}, got {}",
                auction_currency, asset_sent
            ),
        }
    );
    ensure!(
        amount_sent == buy_now_price,
        ContractError::InvalidFunds {
            msg: format!(
                "Incorrect amount: expected {}, got {}",
                buy_now_price, amount_sent
            ),
        }
    );

    let mut cw20_transfer: Vec<WasmMsg> = vec![];
    // Send back previous bid unless there was no previous bid.
    if token_auction_state.high_bidder_amount > Uint128::zero() {
        let transfer_msg = Cw20ExecuteMsg::Transfer {
            recipient: token_auction_state.high_bidder_addr.to_string(),
            amount: token_auction_state.high_bidder_amount,
        };
        let wasm_msg = wasm_execute(auction_currency, &transfer_msg, vec![])?;
        cw20_transfer.push(wasm_msg);
    }
    token_auction_state.high_bidder_addr = sender_addr;
    token_auction_state.high_bidder_amount = amount_sent;
    // Set auction as bought
    token_auction_state.is_bought = true;

    let key = token_auction_state.auction_id.u128();
    TOKEN_AUCTION_STATE.save(deps.storage, key, &token_auction_state)?;

    // Calculate the funds to be received after tax
    let (after_tax_payment, tax_messages) = purchase_token(
        deps.as_ref(),
        &info,
        token_auction_state.clone(),
        action,
        amount_sent,
    )?;

    let mut resp: Response = Response::new()
        // Send NFT to auction winner.
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token_auction_state.token_address.clone(),
            msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                recipient: token_auction_state.high_bidder_addr.to_string(),
                token_id: token_id.clone(),
            })?,
            funds: vec![],
        }))
        // Refund to highest bidder
        .add_messages(cw20_transfer)
        // Send tax/royalty messages
        .add_submessages(tax_messages)
        .add_attribute("action", "buy_now")
        .add_attribute("token_id", token_id.clone())
        .add_attribute("token_contract", token_auction_state.clone().token_address)
        .add_attribute("recipient", &token_auction_state.high_bidder_addr)
        .add_attribute("bought_at", token_auction_state.buy_now_price.unwrap())
        .add_attribute("auction_id", token_auction_state.auction_id);

    let recipient = token_auction_state
        .recipient
        .unwrap_or(Recipient::from_string(token_auction_state.owner));

    match after_tax_payment {
        Funds::Native(native_funds) => {
            // Send payment to recipient
            resp = resp
                .add_submessage(recipient.generate_direct_msg(&deps.as_ref(), vec![native_funds])?)
        }
        Funds::Cw20(cw20_funds) => {
            let cw20_msg = recipient.generate_msg_cw20(&deps.as_ref(), cw20_funds)?;
            resp = resp.add_submessage(cw20_msg)
        }
    }
    Ok(resp)
}

fn execute_cancel(
    ctx: ExecuteContext,
    token_id: String,
    token_address: String,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    nonpayable(&info)?;

    let mut token_auction_state =
        get_existing_token_auction_state(deps.storage, &token_id, &token_address)?;
    ensure!(
        info.sender == token_auction_state.owner,
        ContractError::Unauthorized {}
    );
    ensure!(
        !token_auction_state.end_time.is_expired(&env.block),
        ContractError::AuctionEnded {}
    );
    ensure!(
        !token_auction_state.is_bought,
        ContractError::AuctionBought {}
    );
    let mut messages: Vec<CosmosMsg> = vec![CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_auction_state.token_address.clone(),
        msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
            recipient: info.sender.to_string(),
            token_id,
        })?,
        funds: vec![],
    })];

    // Refund highest bid, if it exists.
    if !token_auction_state.high_bidder_amount.is_zero() {
        let is_cw20_auction = token_auction_state.uses_cw20;
        if is_cw20_auction {
            let auction_currency = token_auction_state.clone().coin_denom;
            let transfer_msg = Cw20ExecuteMsg::Transfer {
                recipient: token_auction_state.high_bidder_addr.clone().into_string(),
                amount: token_auction_state.high_bidder_amount,
            };
            let wasm_msg = wasm_execute(auction_currency, &transfer_msg, vec![])?;
            messages.push(CosmosMsg::Wasm(wasm_msg))
        } else {
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: token_auction_state.high_bidder_addr.to_string(),
                amount: coins(
                    token_auction_state.high_bidder_amount.u128(),
                    token_auction_state.coin_denom.clone(),
                ),
            }));
        }
    }

    token_auction_state.is_cancelled = true;
    TOKEN_AUCTION_STATE.save(
        deps.storage,
        token_auction_state.auction_id.u128(),
        &token_auction_state,
    )?;

    Ok(Response::new().add_messages(messages))
}

fn execute_claim(
    ctx: ExecuteContext,
    token_id: String,
    token_address: String,
    action: String,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let token_auction_state =
        get_existing_token_auction_state(deps.storage, &token_id, &token_address)?;
    ensure!(
        token_auction_state.end_time.is_expired(&env.block),
        ContractError::AuctionNotEnded {}
    );
    let token_owner = query_owner_of(
        deps.querier,
        token_auction_state.token_address.clone(),
        token_id.clone(),
    )?
    .owner;
    ensure!(
        // If this is false then the token is no longer held by the contract so the token has been
        // claimed.
        token_owner == env.contract.address,
        ContractError::AuctionAlreadyClaimed {}
    );
    // This is the case where no-one bid on the token.
    if token_auction_state.high_bidder_addr.to_string().is_empty()
        || token_auction_state.high_bidder_amount.is_zero()
    {
        return Ok(Response::new()
            // Send NFT back to the original owner.
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: token_auction_state.token_address.clone(),
                msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                    recipient: token_auction_state.owner.clone(),
                    token_id: token_id.clone(),
                })?,
                funds: vec![],
            }))
            .add_attribute("action", "claim")
            .add_attribute("token_id", token_id)
            .add_attribute("token_contract", token_auction_state.token_address)
            .add_attribute("recipient", token_auction_state.owner)
            .add_attribute("winning_bid_amount", token_auction_state.high_bidder_amount)
            .add_attribute("auction_id", token_auction_state.auction_id));
    }

    // Calculate the funds to be received after tax
    let (after_tax_payment, tax_messages) = purchase_token(
        deps.as_ref(),
        &info,
        token_auction_state.clone(),
        action,
        token_auction_state.high_bidder_amount,
    )?;

    let mut resp: Response = Response::new()
        // Send NFT to auction winner.
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token_auction_state.token_address.clone(),
            msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                recipient: token_auction_state.high_bidder_addr.to_string(),
                token_id: token_id.clone(),
            })?,
            funds: vec![],
        }))
        // Send tax/royalty messages
        .add_submessages(tax_messages)
        .add_attribute("action", "claim")
        .add_attribute("token_id", token_id.clone())
        .add_attribute("token_contract", token_auction_state.clone().token_address)
        .add_attribute("recipient", &token_auction_state.high_bidder_addr)
        .add_attribute("winning_bid_amount", token_auction_state.high_bidder_amount)
        .add_attribute("auction_id", token_auction_state.auction_id);

    let recipient = token_auction_state
        .recipient
        .unwrap_or(Recipient::from_string(token_auction_state.owner));

    match after_tax_payment {
        Funds::Native(native_funds) => {
            // Send payment to recipient
            resp = resp
                .add_submessage(recipient.generate_direct_msg(&deps.as_ref(), vec![native_funds])?)
        }
        Funds::Cw20(cw20_funds) => {
            let cw20_msg = recipient.generate_msg_cw20(&deps.as_ref(), cw20_funds)?;
            resp = resp.add_submessage(cw20_msg)
        }
    }
    Ok(resp)
}

fn purchase_token(
    deps: Deps,
    _info: &MessageInfo,
    state: TokenAuctionState,
    action: String,
    amount: Uint128,
) -> Result<(Funds, Vec<SubMsg>), ContractError> {
    if !state.uses_cw20 {
        let total_cost = Coin::new(amount.u128(), state.coin_denom.clone());
        let transfer_response = ADOContract::default().query_deducted_funds(
            deps,
            action,
            Funds::Native(total_cost.clone()),
        )?;
        match transfer_response {
            Some(transfer_response) => {
                let remaining_amount = transfer_response.leftover_funds.try_get_coin()?;
                let after_tax_payment = Coin {
                    denom: state.coin_denom,
                    amount: remaining_amount.amount,
                };
                Ok((Funds::Native(after_tax_payment), transfer_response.msgs))
            }
            None => {
                let after_tax_payment = Coin {
                    denom: state.coin_denom,
                    amount: total_cost.amount,
                };
                Ok((Funds::Native(after_tax_payment), vec![]))
            }
        }
    } else {
        let total_cost = Cw20Coin {
            address: state.coin_denom.clone(),
            amount,
        };
        let transfer_response = ADOContract::default().query_deducted_funds(
            deps,
            action,
            Funds::Cw20(total_cost.clone()),
        )?;
        match transfer_response {
            Some(transfer_response) => {
                let remaining_amount = transfer_response.leftover_funds.try_get_cw20_coin()?;
                Ok((Funds::Cw20(remaining_amount), transfer_response.msgs))
            }
            None => {
                let after_tax_payment = Cw20Coin {
                    address: state.coin_denom,
                    amount: total_cost.amount,
                };
                Ok((Funds::Cw20(after_tax_payment), vec![]))
            }
        }
    }
}

fn get_existing_token_auction_state(
    storage: &dyn Storage,
    token_id: &str,
    token_address: &str,
) -> Result<TokenAuctionState, ContractError> {
    let key = token_id.to_owned() + token_address;
    let latest_auction_id: Uint128 = match auction_infos().may_load(storage, &key)? {
        None => return Err(ContractError::AuctionDoesNotExist {}),
        Some(auction_info) => *auction_info.last().unwrap(),
    };
    let token_auction_state = TOKEN_AUCTION_STATE.load(storage, latest_auction_id.u128())?;

    Ok(token_auction_state)
}

fn get_and_increment_next_auction_id(
    storage: &mut dyn Storage,
    token_id: &str,
    token_address: &str,
) -> Result<Uint128, ContractError> {
    let next_auction_id = NEXT_AUCTION_ID.load(storage)?;
    let incremented_next_auction_id = next_auction_id.checked_add(Uint128::from(1u128))?;
    NEXT_AUCTION_ID.save(storage, &incremented_next_auction_id)?;

    let key = token_id.to_owned() + token_address;

    let mut auction_info = auction_infos().load(storage, &key).unwrap_or_default();
    auction_info.push(next_auction_id);
    if auction_info.token_address.is_empty() {
        token_address.clone_into(&mut auction_info.token_address);
        token_id.clone_into(&mut auction_info.token_id);
    }
    auction_infos().save(storage, &key, &auction_info)?;
    Ok(next_auction_id)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::LatestAuctionState {
            token_id,
            token_address,
        } => encode_binary(&query_latest_auction_state(deps, token_id, token_address)?),
        QueryMsg::AuctionState { auction_id } => {
            encode_binary(&query_auction_state(deps, auction_id)?)
        }
        QueryMsg::Bids {
            auction_id,
            start_after,
            limit,
            order_by,
        } => encode_binary(&query_bids(deps, auction_id, start_after, limit, order_by)?),
        QueryMsg::AuctionIds {
            token_id,
            token_address,
        } => encode_binary(&query_auction_ids(deps, token_id, token_address)?),
        QueryMsg::AuctionInfosForAddress {
            token_address,
            start_after,
            limit,
        } => encode_binary(&query_auction_infos_for_address(
            deps,
            token_address,
            start_after,
            limit,
        )?),
        QueryMsg::IsCancelled {
            token_id,
            token_address,
        } => encode_binary(&query_is_cancelled(deps, token_id, token_address)?),
        QueryMsg::IsClaimed {
            token_id,
            token_address,
        } => encode_binary(&query_is_claimed(deps, env, token_id, token_address)?),
        QueryMsg::IsClosed {
            token_id,
            token_address,
        } => encode_binary(&query_is_closed(deps, env, token_id, token_address)?),
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

fn query_is_cancelled(
    deps: Deps,
    token_id: String,
    token_address: String,
) -> Result<IsCancelledResponse, ContractError> {
    let token_auction_state_result =
        get_existing_token_auction_state(deps.storage, &token_id, &token_address);
    if let Ok(token_auction_state) = token_auction_state_result {
        return Ok(IsCancelledResponse {
            is_cancelled: token_auction_state.is_cancelled,
        });
    }
    Err(ContractError::AuctionDoesNotExist {})
}

fn query_is_claimed(
    deps: Deps,
    env: Env,
    token_id: String,
    token_address: String,
) -> Result<IsClaimedResponse, ContractError> {
    let token_auction_state =
        get_existing_token_auction_state(deps.storage, &token_id, &token_address)?;

    let token_owner =
        query_owner_of(deps.querier, token_auction_state.token_address, token_id)?.owner;

    // if token owner isn't the contract, it means that it has been claimed. If they're equal it means that it hasn't been claimed and will return false
    Ok(IsClaimedResponse {
        is_claimed: token_owner != env.contract.address,
    })
}

fn query_is_closed(
    deps: Deps,
    env: Env,
    token_id: String,
    token_address: String,
) -> Result<IsClosedResponse, ContractError> {
    let token_auction_state =
        get_existing_token_auction_state(deps.storage, &token_id, &token_address)?;

    let is_claimed =
        query_is_claimed(deps, env.clone(), token_id.clone(), token_address.clone())?.is_claimed;
    let is_cancelled = query_is_cancelled(deps, token_id, token_address)?.is_cancelled;
    let is_expired = token_auction_state.end_time.is_expired(&env.block);

    // Considers the auction closed if one or more of those 3 variables are true. Otherwise it isn't closed.
    let is_closed = is_claimed || is_cancelled || is_expired;
    Ok(IsClosedResponse { is_closed })
}

fn query_auction_ids(
    deps: Deps,
    token_id: String,
    token_address: String,
) -> Result<AuctionIdsResponse, ContractError> {
    let key = token_id + &token_address;
    let auction_info = auction_infos().may_load(deps.storage, &key)?;
    if let Some(auction_info) = auction_info {
        return Ok(AuctionIdsResponse {
            auction_ids: auction_info.auction_ids,
        });
    }
    Ok(AuctionIdsResponse {
        auction_ids: vec![],
    })
}

pub fn query_auction_infos_for_address(
    deps: Deps,
    token_address: String,
    start_after: Option<String>,
    limit: Option<u64>,
) -> Result<Vec<AuctionInfo>, ContractError> {
    read_auction_infos(deps.storage, token_address, start_after, limit)
}

fn query_bids(
    deps: Deps,
    auction_id: Uint128,
    start_after: Option<u64>,
    limit: Option<u64>,
    order_by: Option<OrderBy>,
) -> Result<BidsResponse, ContractError> {
    let bids = read_bids(
        deps.storage,
        auction_id.u128(),
        start_after,
        limit,
        order_by,
    )?;
    Ok(BidsResponse { bids })
}

fn query_latest_auction_state(
    deps: Deps,
    token_id: String,
    token_address: String,
) -> Result<AuctionStateResponse, ContractError> {
    let token_auction_state_result =
        get_existing_token_auction_state(deps.storage, &token_id, &token_address);
    if let Ok(token_auction_state) = token_auction_state_result {
        return Ok(token_auction_state.into());
    }
    Err(ContractError::AuctionDoesNotExist {})
}

fn query_auction_state(
    deps: Deps,
    auction_id: Uint128,
) -> Result<AuctionStateResponse, ContractError> {
    let token_auction_state = TOKEN_AUCTION_STATE.load(deps.storage, auction_id.u128())?;
    Ok(token_auction_state.into())
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
