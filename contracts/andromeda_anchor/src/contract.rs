use crate::state::{
    Config, Position, CONFIG, POSITION, PREV_AUST_BALANCE, PREV_UUSD_BALANCE, RECIPIENT_ADDR,
};
use andromeda_protocol::{
    anchor::{ExecuteMsg, InstantiateMsg, PositionResponse, QueryMsg, WithdrawalType},
    communication::{encode_binary, parse_message, AndromedaMsg, AndromedaQuery, Recipient},
    error::ContractError,
    operators::{execute_update_operators, is_operator, query_is_operator, query_operators},
    ownership::{execute_update_owner, is_contract_owner, query_contract_owner, CONTRACT_OWNER},
    require,
};
use cosmwasm_std::{
    attr, coins, entry_point, to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Reply,
    Response, SubMsg, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

use terraswap::querier::{query_balance, query_token_balance};

use andromeda_protocol::anchor::{AnchorMarketMsg, ConfigResponse};

const UUSD_DENOM: &str = "uusd";
pub const DEPOSIT_ID: u64 = 1;
pub const WITHDRAW_ID: u64 = 2;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        anchor_market: deps.api.addr_canonicalize(&msg.anchor_market)?,
        aust_token: deps.api.addr_canonicalize(&msg.aust_token)?,
    };
    CONFIG.save(deps.storage, &config)?;
    PREV_AUST_BALANCE.save(deps.storage, &Uint128::zero())?;
    PREV_UUSD_BALANCE.save(deps.storage, &Uint128::zero())?;
    CONTRACT_OWNER.save(deps.storage, &info.sender)?;
    Ok(Response::new().add_attributes(vec![attr("action", "instantiate"), attr("type", "anchor")]))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AndrReceive(msg) => execute_andr_receive(deps, env, info, msg),
        ExecuteMsg::Deposit { recipient } => execute_deposit(deps, env, info, recipient),
        ExecuteMsg::Withdraw {
            withdrawal_type,
            recipient_addr,
        } => execute_withdraw(deps, env, info, withdrawal_type, recipient_addr),
    }
}

fn execute_andr_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: AndromedaMsg,
) -> Result<Response, ContractError> {
    match msg {
        AndromedaMsg::Receive(data) => {
            let received: ExecuteMsg = parse_message(data)?;
            match received {
                ExecuteMsg::AndrReceive(..) => Err(ContractError::NestedAndromedaMsg {}),
                _ => execute(deps, env, info, received),
            }
        }
        AndromedaMsg::UpdateOwner { address } => execute_update_owner(deps, info, address),
        AndromedaMsg::UpdateOperators { operators } => {
            execute_update_operators(deps, info, operators)
        }
        AndromedaMsg::Withdraw { .. } => Err(ContractError::UnsupportedOperation {}),
    }
}

pub fn execute_deposit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: Option<Recipient>,
) -> Result<Response, ContractError> {
    require(
        info.funds.len() == 1,
        ContractError::InvalidFunds {
            msg: "Must deposit exactly 1 type of native coin.".to_string(),
        },
    )?;

    let config = CONFIG.load(deps.storage)?;
    let recipient = match recipient {
        Some(recipient) => recipient,
        None => Recipient::Addr(info.sender.to_string()),
    };

    let payment = &info.funds[0];
    require(
        payment.denom == UUSD_DENOM && payment.amount > Uint128::zero(),
        ContractError::InvalidFunds {
            msg: "Must deposit a non-zero quantity of uusd".to_string(),
        },
    )?;

    let aust_balance = query_token_balance(
        &deps.querier,
        deps.api.addr_humanize(&config.aust_token)?,
        env.contract.address,
    )?;
    let recipient_addr = recipient.get_addr();
    PREV_AUST_BALANCE.save(deps.storage, &aust_balance)?;
    RECIPIENT_ADDR.save(deps.storage, &recipient_addr)?;
    let payment_amount = payment.amount;

    if !POSITION.has(deps.storage, &recipient_addr) {
        POSITION.save(
            deps.storage,
            &recipient_addr,
            &Position {
                recipient,
                aust_amount: Uint128::zero(),
            },
        )?;
    }

    //deposit Anchor Mint
    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.addr_humanize(&config.anchor_market)?.to_string(),
                msg: to_binary(&AnchorMarketMsg::DepositStable {})?,
                funds: vec![payment.clone()],
            }),
            DEPOSIT_ID,
        ))
        .add_attributes(vec![
            attr("action", "deposit"),
            attr("deposit_amount", payment_amount),
        ]))
}

pub fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    withdrawal_type: Option<WithdrawalType>,
    recipient_addr: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let recipient_addr = recipient_addr.unwrap_or_else(|| info.sender.to_string());
    let mut position = POSITION.load(deps.storage, &recipient_addr)?;

    let authorized = recipient_addr == info.sender
        || is_operator(deps.storage, info.sender.as_str())?
        || is_contract_owner(deps.storage, info.sender.as_str())?;

    require(authorized, ContractError::Unauthorized {})?;

    let contract_balance =
        query_balance(&deps.querier, env.contract.address, UUSD_DENOM.to_owned())?;
    PREV_UUSD_BALANCE.save(deps.storage, &contract_balance)?;
    RECIPIENT_ADDR.save(deps.storage, &recipient_addr)?;
    let amount_to_redeem = match withdrawal_type {
        None => position.aust_amount,
        Some(withdrawal_type) => match withdrawal_type {
            WithdrawalType::Percentage(percent) => {
                require(percent <= 100u128.into(), ContractError::InvalidRate {})?;
                position.aust_amount.multiply_ratio(percent, 100u128)
            }
            WithdrawalType::Amount(amount) => {
                require(
                    amount <= position.aust_amount,
                    ContractError::InvalidFunds {
                        msg: "Requested withdrawal amount exceeds amount of aUST in position"
                            .to_string(),
                    },
                )?;
                amount
            }
        },
    };
    position.aust_amount = position.aust_amount.checked_sub(amount_to_redeem)?;
    POSITION.save(deps.storage, &recipient_addr, &position)?;

    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.addr_humanize(&config.aust_token)?.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: deps.api.addr_humanize(&config.anchor_market)?.to_string(),
                    amount: amount_to_redeem,
                    msg: to_binary(&AnchorMarketMsg::RedeemStable {})?,
                })?,
                funds: vec![],
            }),
            WITHDRAW_ID,
        ))
        .add_attributes(vec![
            attr("action", "withdraw"),
            attr("recipient_addr", recipient_addr),
        ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        DEPOSIT_ID => reply_update_position(deps, env),
        WITHDRAW_ID => reply_withdraw_ust(deps, env),
        _ => Err(ContractError::InvalidReplyId {}),
    }
}

fn reply_update_position(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    // stores aUST amount to position
    let config = CONFIG.load(deps.storage)?;
    let aust_balance = query_token_balance(
        &deps.querier,
        deps.api.addr_humanize(&config.aust_token)?,
        env.contract.address,
    )?;

    let prev_aust_balance = PREV_AUST_BALANCE.load(deps.storage)?;
    let new_aust_balance = aust_balance.checked_sub(prev_aust_balance)?;
    require(
        new_aust_balance > Uint128::zero(),
        ContractError::InvalidFunds {
            msg: "No aUST tokens minted".to_string(),
        },
    )?;

    let recipient_addr = RECIPIENT_ADDR.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &recipient_addr)?;
    position.aust_amount += new_aust_balance;
    POSITION.save(deps.storage, &recipient_addr, &position)?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "reply_update_position"),
        attr("recipient_addr", recipient_addr.clone()),
        attr("aust_amount", new_aust_balance.to_string()),
    ]))
}

fn reply_withdraw_ust(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let current_balance =
        query_balance(&deps.querier, env.contract.address, UUSD_DENOM.to_owned())?;
    let prev_balance = PREV_UUSD_BALANCE.load(deps.storage)?;
    let transfer_amount = current_balance - prev_balance;

    let recipient_addr = RECIPIENT_ADDR.load(deps.storage)?;
    let recipient = POSITION.load(deps.storage, &recipient_addr)?.recipient;
    let mut msgs = vec![];
    if transfer_amount > Uint128::zero() {
        msgs.push(
            recipient
                .generate_msg_native(&deps.as_ref(), coins(transfer_amount.u128(), UUSD_DENOM))?,
        );
    }
    Ok(Response::new()
        .add_submessages(msgs)
        .add_attribute("action", "reply_withdraw_ust")
        .add_attribute("recipient", recipient_addr)
        .add_attribute("amount", transfer_amount))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::AndrQuery(msg) => handle_andromeda_query(deps, msg),
        QueryMsg::Config {} => encode_binary(&query_config(deps)?),
    }
}

fn handle_andromeda_query(deps: Deps, msg: AndromedaQuery) -> Result<Binary, ContractError> {
    match msg {
        AndromedaQuery::Get(data) => {
            let recipient: String = parse_message(data)?;
            encode_binary(&query_position(deps, recipient)?)
        }
        AndromedaQuery::Owner {} => encode_binary(&query_contract_owner(deps)?),
        AndromedaQuery::Operators {} => encode_binary(&query_operators(deps)?),
        AndromedaQuery::IsOperator { address } => {
            encode_binary(&query_is_operator(deps, &address)?)
        }
    }
}

fn query_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        anchor_market: deps.api.addr_humanize(&config.anchor_market)?.to_string(),
        aust_token: deps.api.addr_humanize(&config.aust_token)?.to_string(),
    })
}

fn query_position(deps: Deps, recipient: String) -> Result<PositionResponse, ContractError> {
    let position = POSITION.load(deps.storage, &recipient)?;
    Ok(PositionResponse {
        recipient: position.recipient,
        aust_amount: position.aust_amount,
    })
}
