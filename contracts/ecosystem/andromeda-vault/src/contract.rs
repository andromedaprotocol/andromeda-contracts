use andromeda_ecosystem::vault::{
    DepositMsg, ExecuteMsg, InstantiateMsg, QueryMsg, StrategyAddressResponse, StrategyType,
    BALANCES, STRATEGY_CONTRACT_ADDRESSES,
};
use andromeda_std::ado_base::ownership::ContractOwnerResponse;
use andromeda_std::ado_contract::ADOContract;
use andromeda_std::amp::{AndrAddr, Recipient};
use andromeda_std::common::actions::call_action;
use andromeda_std::common::context::ExecuteContext;
use andromeda_std::common::encode_binary;
use andromeda_std::{
    ado_base::withdraw::{Withdrawal, WithdrawalType},
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    error::ContractError,
};

use cosmwasm_std::{
    attr, coin, ensure, entry_point, from_json, to_json_binary, BankMsg, Binary, Coin,
    ContractResult, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo, Order, QueryRequest, Reply,
    ReplyOn, Response, StdError, SubMsg, SystemResult, Uint128, WasmMsg, WasmQuery,
};
use cw_utils::nonpayable;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-vault";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        &deps.querier,
        info,
        BaseInstantiateMsg {
            ado_type: CONTRACT_NAME.to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            owner: msg.owner,
            kernel_address: msg.kernel_address,
        },
    )
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
    call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;
    match msg {
        ExecuteMsg::Deposit { recipient, msg } => execute_deposit(ctx, recipient, msg),
        ExecuteMsg::WithdrawVault {
            recipient,
            withdrawals,
            strategy,
        } => execute_withdraw(ctx, recipient, withdrawals, strategy),
        ExecuteMsg::UpdateStrategy { strategy, address } => {
            execute_update_strategy(ctx, strategy, address)
        }
        ExecuteMsg::Withdraw { .. } => Err(ContractError::NotImplemented {
            msg: Some("Please use WithdrawVault".to_string()),
        }),
        _ => ADOContract::default().execute(ctx, msg),
    }
}

fn execute_deposit(
    ctx: ExecuteContext,
    recipient: Option<AndrAddr>,
    msg: Option<Binary>,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;
    let DepositMsg {
        strategy,
        amount,
        deposit_msg,
    } = match msg {
        None => DepositMsg::default(),
        Some(msg) => from_json(msg)?,
    };
    let mut resp = Response::default();

    let recipient = recipient.unwrap_or_else(|| AndrAddr::from_string(info.sender.to_string()));

    // Validate address
    let recipient_addr = recipient.get_raw_address(&deps.as_ref())?;
    // If no amount is provided then the sent funds are used as a deposit
    let deposited_funds = if let Some(deposit_amount) = amount {
        ensure!(
            !deposit_amount.amount.is_zero(),
            ContractError::InsufficientFunds {}
        );
        let mut deposit_balance = info
            .funds
            .iter()
            .find(|f| f.denom == deposit_amount.denom.clone())
            .unwrap_or(&Coin {
                denom: deposit_amount.denom.clone(),
                amount: Uint128::zero(),
            })
            .amount;

        // If depositing to a yield strategy we must first check that the sender has either provided the amount to deposit
        // or has a combination of the amount within the vault and within the sent funds
        if strategy.is_some() && deposit_balance <= deposit_amount.amount {
            let balance_key = (recipient_addr.as_str(), deposit_amount.denom.as_str());
            let vault_balance = BALANCES
                .may_load(deps.storage, balance_key)?
                .unwrap_or_else(Uint128::zero);

            // Amount that must be removed from the vault to add to the deposit
            let difference = deposit_amount.amount.checked_sub(deposit_balance)?;
            ensure!(
                vault_balance >= difference,
                ContractError::InsufficientFunds {}
            );

            deposit_balance = deposit_balance.checked_add(vault_balance)?;
            //Subtract the removed funds from balance
            BALANCES.save(
                deps.storage,
                balance_key,
                &vault_balance.checked_sub(difference)?,
            )?;
        }

        // Ensure enough funds are present
        ensure!(
            deposit_balance >= deposit_amount.amount,
            ContractError::InsufficientFunds {}
        );
        let funds_vec: Vec<Coin> = vec![deposit_amount];
        funds_vec
    } else {
        info.funds.to_vec()
    };

    ensure!(
        !deposited_funds.is_empty(),
        ContractError::InsufficientFunds {}
    );
    match strategy {
        // Depositing to vault
        None => {
            for funds in deposited_funds {
                let balance_key = (recipient_addr.as_str(), funds.denom.as_str());
                let curr_balance = BALANCES
                    .may_load(deps.storage, balance_key)?
                    .unwrap_or_default();
                BALANCES.save(
                    deps.storage,
                    balance_key,
                    &curr_balance.checked_add(funds.amount)?,
                )?;
            }
        }
        Some(strategy) => {
            let mut deposit_msgs: Vec<SubMsg> = Vec::new();
            for funds in deposited_funds {
                let deposit_msg = strategy.deposit(
                    deps.storage,
                    funds,
                    Recipient::new(recipient.clone(), deposit_msg.clone()),
                )?;
                deposit_msgs.push(deposit_msg);
            }
            resp = resp.add_submessages(deposit_msgs)
        }
    }
    Ok(resp.add_attributes(vec![
        attr("action", "deposit"),
        attr("recipient", recipient_addr),
    ]))
}

pub fn execute_withdraw(
    ctx: ExecuteContext,
    recipient: Option<Recipient>,
    withdrawals: Vec<Withdrawal>,
    strategy: Option<StrategyType>,
) -> Result<Response, ContractError> {
    let ExecuteContext { info, deps, .. } = ctx;
    nonpayable(&info)?;

    ensure!(
        !withdrawals.is_empty(),
        ContractError::InvalidTokensToWithdraw {
            msg: "No tokens provided for withdrawal".to_string(),
        }
    );
    match strategy {
        None => withdraw_vault(deps, info, recipient, withdrawals),
        Some(strategy) => withdraw_strategy(deps, info, strategy, withdrawals),
    }
}

pub fn withdraw_vault(
    deps: DepsMut,
    info: MessageInfo,
    recipient: Option<Recipient>,
    withdrawals: Vec<Withdrawal>,
) -> Result<Response, ContractError> {
    let res = Response::default();
    let mut withdrawal_amount: Vec<Coin> = Vec::new();

    let recipient = recipient
        .unwrap_or_else(|| Recipient::from_string(info.sender.to_string()))
        .address
        .get_raw_address(&deps.as_ref())?;
    for withdrawal in withdrawals {
        let denom = withdrawal.token;
        let balance = BALANCES
            .load(deps.storage, (info.sender.as_str(), &denom))
            .unwrap_or_else(|_| Uint128::zero());
        ensure!(!balance.is_zero(), ContractError::InsufficientFunds {});

        match withdrawal.withdrawal_type {
            Some(withdrawal_type) => match withdrawal_type {
                WithdrawalType::Amount(amount) => {
                    ensure!(
                        !amount.is_zero(),
                        ContractError::InvalidWithdrawal {
                            msg: Some("Amount must be non-zero".to_string()),
                        }
                    );
                    ensure!(balance >= amount, ContractError::InsufficientFunds {});
                    withdrawal_amount.push(coin(amount.u128(), denom.clone()));
                    BALANCES.save(
                        deps.storage,
                        (info.sender.as_str(), &denom),
                        &balance.checked_sub(amount)?,
                    )?;
                }
                WithdrawalType::Percentage(percent) => {
                    ensure!(
                        !percent.is_zero(),
                        ContractError::InvalidWithdrawal {
                            msg: Some("Percent must be non-zero".to_string()),
                        }
                    );
                    let amount = balance * percent;
                    withdrawal_amount.push(coin(amount.u128(), denom.clone()));
                    BALANCES.save(
                        deps.storage,
                        (info.sender.as_str(), &denom),
                        &balance.checked_sub(amount)?,
                    )?;
                }
            },
            None => {
                withdrawal_amount.push(coin(balance.u128(), denom.clone()));
                BALANCES.save(
                    deps.storage,
                    (info.sender.as_str(), &denom),
                    &Uint128::zero(),
                )?;
            }
        }
    }
    let withdrawal_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: recipient.into(),
        amount: withdrawal_amount,
    });
    Ok(res.add_message(withdrawal_msg))
}

pub fn withdraw_strategy(
    deps: DepsMut,
    info: MessageInfo,
    strategy: StrategyType,
    withdrawals: Vec<Withdrawal>,
) -> Result<Response, ContractError> {
    let res = Response::default();
    let recipient = Recipient::from_string(info.sender.to_string());
    let addr_opt = STRATEGY_CONTRACT_ADDRESSES.may_load(deps.storage, strategy.to_string())?;
    if addr_opt.is_none() {
        return Err(ContractError::InvalidStrategy {
            strategy: strategy.to_string(),
        });
    }

    let addr = addr_opt.unwrap();
    let withdraw_exec = to_json_binary(&ExecuteMsg::Withdraw {
        recipient: Some(recipient),
        tokens_to_withdraw: Some(withdrawals),
    })?;
    let withdraw_submsg = SubMsg {
        id: 104,
        msg: CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: addr,
            msg: withdraw_exec,
            funds: vec![],
        }),
        gas_limit: None,
        reply_on: ReplyOn::Error,
    };

    Ok(res.add_submessage(withdraw_submsg))
}

fn execute_update_strategy(
    ctx: ExecuteContext,
    strategy: StrategyType,
    address: AndrAddr,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, env, info, ..
    } = ctx;
    ensure!(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_ref())?,
        ContractError::Unauthorized {}
    );
    let strategy_addr = address.get_raw_address(&deps.as_ref())?;

    //The vault contract must be an operator for the given contract in order to enable withdrawals
    //DEV: with custom approval functionality this check can be removed
    // let strategy_is_operator: IsOperatorResponse = query_get(
    //     Some(to_json_binary(&AndromedaQuery::IsOperator {
    //         address: env.contract.address.to_string(),
    //     })?),
    //     strategy_addr.clone(),
    //     &deps.querier,
    // )?;

    // Replaced operator with owner check
    // let strategy_is_operator: IsOperatorResponse = deps.querier.query_wasm_smart(
    //     strategy_addr.clone(),
    //     &QueryMsg::IsOperator {
    //         address: env.contract.address.to_string(),
    //     },
    // )?;

    let strategy_owner: ContractOwnerResponse = deps
        .querier
        .query_wasm_smart(strategy_addr.clone(), &QueryMsg::Owner {})?;

    ensure!(
        strategy_owner.owner == env.contract.address,
        ContractError::NotAssignedOperator {
            msg: Some("Vault contract is not an operator for the given address".to_string()),
        }
    );

    STRATEGY_CONTRACT_ADDRESSES.save(
        deps.storage,
        strategy.to_string(),
        &strategy_addr.to_string(),
    )?;

    Ok(Response::default()
        .add_attribute("action", "update_strategy")
        .add_attribute("strategy_type", strategy.to_string())
        .add_attribute("addr", strategy_addr))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, env, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::VaultBalance {
            address,
            strategy,
            denom,
        } => query_balance(deps, address, strategy, denom),
        QueryMsg::StrategyAddress { strategy } => {
            encode_binary(&query_strategy_address(deps, env, strategy)?)
        }
        _ => ADOContract::default().query(deps, env, msg),
    }
}

fn query_balance(
    deps: Deps,
    address: AndrAddr,
    strategy: Option<StrategyType>,
    denom: Option<String>,
) -> Result<Binary, ContractError> {
    if let Some(strategy) = strategy {
        let strategy_addr = STRATEGY_CONTRACT_ADDRESSES.load(deps.storage, strategy.to_string())?;
        ensure!(false, ContractError::TemporarilyDisabled {});
        // DEV NOTE: Why does this ensure! a generic type when not using custom query?
        // let query: QueryRequest<Empty> = QueryRequest::Wasm(WasmQuery::Smart {
        //     contract_addr: strategy_addr,
        //     msg: to_json_binary(&AndromedaQuery::WithdrawableBalance { address })?,
        // });
        // TODO: Below code to be replaced with above code once WithdrawableBalance is re-enabled
        let query: QueryRequest<Empty> = QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: strategy_addr,
            msg: to_json_binary(&Binary::default())?,
        });
        match deps.querier.raw_query(&to_json_binary(&query)?) {
            SystemResult::Ok(ContractResult::Ok(value)) => Ok(value),
            _ => Err(ContractError::InvalidQuery {}),
        }
    } else if let Some(denom) = denom {
        let balance = BALANCES.load(
            deps.storage,
            ((address.get_raw_address(&deps)?.as_str()), denom.as_str()),
        )?;
        Ok(to_json_binary(&[Coin {
            denom,
            amount: balance,
        }])?)
    } else {
        let balances: Result<Vec<Coin>, ContractError> = BALANCES
            .prefix(address.get_raw_address(&deps)?.as_str())
            .range(deps.storage, None, None, Order::Ascending)
            .map(|v| {
                let (denom, balance) = v?;
                Ok(Coin {
                    denom,
                    amount: balance,
                })
            })
            .collect();
        Ok(to_json_binary(&balances?)?)
    }
}

fn query_strategy_address(
    deps: Deps,
    _env: Env,
    strategy: StrategyType,
) -> Result<StrategyAddressResponse, ContractError> {
    let addr = STRATEGY_CONTRACT_ADDRESSES.may_load(deps.storage, strategy.to_string())?;
    match addr {
        Some(addr) => Ok(StrategyAddressResponse {
            address: addr,
            strategy,
        }),
        None => Err(ContractError::InvalidStrategy {
            strategy: strategy.to_string(),
        }),
    }
}
