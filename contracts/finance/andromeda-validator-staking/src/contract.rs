use crate::{
    state::{DEFAULT_VALIDATOR, RESTAKING_QUEUE, UNSTAKING_QUEUE},
    util::decode_unstaking_response_data,
};
use cosmwasm_std::{
    coin, ensure, entry_point, Addr, BankMsg, Binary, CosmosMsg, Deps, DepsMut, DistributionMsg,
    Env, FullDelegation, MessageInfo, Reply, Response, StakingMsg, SubMsg, Timestamp, Uint128,
};
use cw2::set_contract_version;

use andromeda_finance::validator_staking::{
    is_validator, ExecuteMsg, GetDefaultValidatorResponse, InstantiateMsg, QueryMsg,
    UnstakingTokens, RESTAKING_ACTION,
};

use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    ado_contract::ADOContract,
    amp::AndrAddr,
    andr_execute_fn,
    common::{context::ExecuteContext, distribution::MsgWithdrawDelegatorReward, encode_binary},
    error::ContractError,
    os::aos_querier::AOSQuerier,
};
use enum_repr::EnumRepr;

use chrono::DateTime;

const CONTRACT_NAME: &str = "crates.io:andromeda-validator-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[EnumRepr(type = "u64")]
pub enum ReplyId {
    ValidatorUnstake = 201,
    SetWithdrawAddress = 202,
    RestakeReward = 203,
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    msg.validate(&deps)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    DEFAULT_VALIDATOR.save(deps.storage, &msg.default_validator)?;

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
    Ok(inst_resp)
}

#[andr_execute_fn]
pub fn execute(ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Stake { validator } => execute_stake(ctx, validator),
        ExecuteMsg::Unstake { validator, amount } => execute_unstake(ctx, validator, amount),
        ExecuteMsg::Claim { validator, restake } => execute_claim(ctx, validator, restake),
        ExecuteMsg::Redelegate {
            src_validator,
            dst_validator,
            amount,
        } => execute_redelegate(ctx, src_validator, dst_validator, amount),
        ExecuteMsg::WithdrawFunds { denom, recipient } => {
            execute_withdraw_fund(ctx, denom, recipient)
        }
        ExecuteMsg::UpdateDefaultValidator { validator } => {
            execute_update_default_validator(ctx, validator)
        }

        _ => ADOContract::default().execute(ctx, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::StakedTokens { validator } => {
            encode_binary(&query_staked_tokens(deps, env.contract.address, validator)?)
        }
        QueryMsg::UnstakedTokens {} => encode_binary(&query_unstaked_tokens(deps)?),

        QueryMsg::DefaultValidator {} => encode_binary(&query_default_validator(deps)?),

        _ => ADOContract::default().query(deps, env, msg),
    }
}

fn execute_stake(ctx: ExecuteContext, validator: Option<Addr>) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;

    // Ensure only one type of coin is received
    ensure!(
        info.funds.len() == 1,
        ContractError::ExceedsMaxAllowedCoins {}
    );

    let default_validator = DEFAULT_VALIDATOR.load(deps.storage)?;

    // Use default validator if validator is not specified by stake msg
    let validator = validator.unwrap_or(default_validator);

    // Check if the validator is valid before staking
    is_validator(&deps, &validator)?;

    // Delegate funds to the validator

    let funds = &info.funds[0];

    let res = Response::new()
        .add_message(StakingMsg::Delegate {
            validator: validator.to_string(),
            amount: funds.clone(),
        })
        .add_attribute("action", "validator-stake")
        .add_attribute("from", info.sender)
        .add_attribute("to", validator.to_string())
        .add_attribute("amount", funds.amount);

    Ok(res)
}

fn execute_redelegate(
    ctx: ExecuteContext,
    src_validator: Option<Addr>,
    dst_validator: Addr,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, env, .. } = ctx;

    let src_validator = match src_validator {
        Some(addr) => {
            is_validator(&deps, &addr)?;
            addr
        }
        None => DEFAULT_VALIDATOR.load(deps.storage)?,
    };

    // Check if the destination validator is valid
    is_validator(&deps, &dst_validator)?;

    // Get redelegation amount
    let Some(full_delegation) = deps
        .querier
        .query_delegation(env.contract.address.to_string(), src_validator.to_string())?
    else {
        return Err(ContractError::InvalidValidatorOperation {
            operation: "Redelegate".to_string(),
            validator: src_validator.to_string(),
        });
    };
    let redelegation_amount = match amount {
        Some(amount) => {
            if amount > full_delegation.can_redelegate.amount {
                return Err(ContractError::InvalidRedelegationAmount {
                    amount: amount.to_string(),
                    max: full_delegation.can_redelegate.amount.to_string(),
                });
            }
            amount
        }
        None => full_delegation.can_redelegate.amount,
    };

    let res = Response::new()
        .add_message(StakingMsg::Redelegate {
            src_validator: src_validator.clone().into_string(),
            dst_validator: dst_validator.clone().into_string(),
            amount: coin(
                redelegation_amount.u128(),
                full_delegation.can_redelegate.denom,
            ),
        })
        .add_attribute("action", "redelegation")
        .add_attribute("from", src_validator.to_string())
        .add_attribute("to", dst_validator.to_string())
        .add_attribute("amount", redelegation_amount.to_string());

    Ok(res)
}

fn execute_unstake(
    ctx: ExecuteContext,
    validator: Option<Addr>,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let delegator = env.contract.address;
    let default_validator = DEFAULT_VALIDATOR.load(deps.storage)?;
    let validator = validator.unwrap_or(default_validator);

    // Check if the validator is valid before unstaking
    is_validator(&deps, &validator)?;

    let Some(res) = deps
        .querier
        .query_delegation(delegator.to_string(), validator.to_string())?
    else {
        return Err(ContractError::InvalidValidatorOperation {
            operation: "Unstake".to_string(),
            validator: validator.to_string(),
        });
    };

    let unstake_amount = amount.unwrap_or(res.amount.amount);

    ensure!(
        !unstake_amount.is_zero() && unstake_amount <= res.amount.amount,
        ContractError::InvalidValidatorOperation {
            operation: "Unstake".to_string(),
            validator: validator.to_string(),
        }
    );

    let fund = coin(unstake_amount.u128(), res.amount.denom);
    let undelegate_msg = CosmosMsg::Staking(StakingMsg::Undelegate {
        validator: validator.to_string(),
        amount: fund.clone(),
    });

    let mut unstaking_queue = UNSTAKING_QUEUE.load(deps.storage).unwrap_or_default();
    unstaking_queue.push(UnstakingTokens {
        fund,
        payout_at: Timestamp::default(),
    });

    UNSTAKING_QUEUE.save(deps.storage, &unstaking_queue)?;

    let undelegate_msg = SubMsg::reply_on_success(undelegate_msg, ReplyId::ValidatorUnstake.repr());

    let res = Response::new()
        .add_submessage(undelegate_msg)
        .add_attribute("action", "validator-unstake")
        .add_attribute("amount", unstake_amount)
        .add_attribute("from", info.sender)
        .add_attribute("to", validator.to_string());

    Ok(res)
}

fn execute_claim(
    ctx: ExecuteContext,
    validator: Option<Addr>,
    restake: Option<bool>,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        mut deps,
        info,
        env,
        ..
    } = ctx;

    let default_validator = DEFAULT_VALIDATOR.load(deps.storage)?;
    let validator = validator.unwrap_or(default_validator);

    // Check if the validator is valid before unstaking
    is_validator(&deps, &validator)?;

    let delegator = env.clone().contract.address;
    let Some(res) = deps
        .querier
        .query_delegation(delegator.to_string(), validator.to_string())?
    else {
        return Err(ContractError::InvalidValidatorOperation {
            operation: "Claim".to_string(),
            validator: validator.to_string(),
        });
    };

    // No reward to claim exist
    ensure!(
        !res.accumulated_rewards.is_empty(),
        ContractError::InvalidClaim {}
    );

    let kernel_addr = ADOContract::default().get_kernel_address(deps.storage)?;
    let curr_chain = AOSQuerier::get_current_chain(&deps.querier, &kernel_addr)?;

    let withdraw_msg: CosmosMsg = if curr_chain == "andromeda" {
        MsgWithdrawDelegatorReward {
            delegator_address: delegator.to_string(),
            validator_address: validator.to_string(),
        }
        .into()
    } else {
        DistributionMsg::WithdrawDelegatorReward {
            validator: validator.to_string(),
        }
        .into()
    };
    let restake = restake.unwrap_or(false);
    // Only one denom is allowed to be restaked at a time
    let res = if restake && res.accumulated_rewards.len() == 1 {
        // Only the contract owner and permissioned actors can restake
        ADOContract::default().is_permissioned(
            deps.branch(),
            env,
            RESTAKING_ACTION,
            info.sender,
        )?;
        RESTAKING_QUEUE.save(deps.storage, &res)?;
        Response::new()
            .add_submessage(SubMsg::reply_always(
                withdraw_msg,
                ReplyId::RestakeReward.repr(),
            ))
            .add_attribute("action", "validator-claim-reward")
            .add_attribute("validator", validator.to_string())
    } else {
        // Ensure msg sender is the contract owner
        ensure!(
            ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
            ContractError::Unauthorized {}
        );
        Response::new()
            .add_message(withdraw_msg)
            .add_attribute("action", "validator-claim-reward")
            .add_attribute("validator", validator.to_string())
    };
    Ok(res)
}

fn execute_withdraw_fund(
    ctx: ExecuteContext,
    denom: Option<String>,
    recipient: Option<AndrAddr>,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;

    let recipient = recipient.map_or(Ok(info.sender), |r| r.get_raw_address(&deps.as_ref()))?;
    let funds = denom.map_or(
        deps.querier
            .query_all_balances(env.contract.address.clone())?,
        |d| {
            deps.querier
                .query_balance(env.contract.address.clone(), d)
                .map(|fund| vec![fund])
                .expect("Invalid denom")
        },
    );

    // Remove expired unstaking requests
    let mut unstaking_queue = UNSTAKING_QUEUE.load(deps.storage)?;
    unstaking_queue.retain(|token| token.payout_at > env.block.time);
    UNSTAKING_QUEUE.save(deps.storage, &unstaking_queue)?;

    ensure!(
        !funds.is_empty(),
        ContractError::InvalidWithdrawal {
            msg: Some("No funds to withdraw".to_string())
        }
    );

    let res = Response::new()
        .add_message(BankMsg::Send {
            to_address: recipient.to_string(),
            amount: funds,
        })
        .add_attribute("action", "withdraw-funds")
        .add_attribute("from", env.contract.address)
        .add_attribute("to", recipient.into_string());

    Ok(res)
}

fn execute_update_default_validator(
    ctx: ExecuteContext,
    validator: Addr,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, .. } = ctx;

    // Check if the validator is valid before setting to default validator
    is_validator(&deps, &validator)?;

    DEFAULT_VALIDATOR.save(deps.storage, &validator)?;

    let res = Response::new()
        .add_attribute("action", "update-default-validator")
        .add_attribute("default_validator", validator.into_string());

    Ok(res)
}

fn query_staked_tokens(
    deps: Deps,
    delegator: Addr,
    validator: Option<Addr>,
) -> Result<FullDelegation, ContractError> {
    let default_validator = DEFAULT_VALIDATOR.load(deps.storage)?;

    // Use default validator if validator is not specified
    let validator = validator.unwrap_or(default_validator);

    let Some(res) = deps
        .querier
        .query_delegation(delegator.to_string(), validator.to_string())?
    else {
        return Err(ContractError::InvalidDelegation {});
    };
    Ok(res)
}

fn query_unstaked_tokens(deps: Deps) -> Result<Vec<UnstakingTokens>, ContractError> {
    let res = UNSTAKING_QUEUE.load(deps.storage)?;
    Ok(res)
}

fn query_default_validator(deps: Deps) -> Result<GetDefaultValidatorResponse, ContractError> {
    let default_validator = DEFAULT_VALIDATOR.load(deps.storage)?;
    Ok(GetDefaultValidatorResponse { default_validator })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match ReplyId::from_repr(msg.id) {
        Some(ReplyId::ValidatorUnstake) => on_validator_unstake(deps, msg),
        Some(ReplyId::RestakeReward) => on_restake_reward(deps, env, msg),
        _ => Ok(Response::default()),
    }
}

pub fn on_validator_unstake(deps: DepsMut, msg: Reply) -> Result<Response, ContractError> {
    let res = msg.result.unwrap();
    let mut unstaking_queue = UNSTAKING_QUEUE.load(deps.storage).unwrap_or_default();
    let payout_at = if res.data.is_some() {
        let data = res.data;
        let (seconds, nanos) = decode_unstaking_response_data(data.unwrap());
        let payout_at = Timestamp::from_seconds(seconds);
        payout_at.plus_nanos(nanos)
    } else {
        let attributes = &res
            .events
            .first()
            .ok_or(ContractError::EmptyEvents {})?
            .attributes;
        let mut payout_at = Timestamp::default();
        for attr in attributes {
            if attr.key == "completion_time" {
                let completion_time = DateTime::parse_from_rfc3339(&attr.value).unwrap();
                let seconds = completion_time.timestamp() as u64;
                let nanos = completion_time.timestamp_subsec_nanos() as u64;
                payout_at = Timestamp::from_seconds(seconds);
                payout_at = payout_at.plus_nanos(nanos);
            }
        }
        payout_at
    };
    let mut unstake_req = unstaking_queue
        .pop()
        .ok_or(ContractError::EmptyUnstakingQueue {})?;
    unstake_req.payout_at = payout_at;

    unstaking_queue.push(unstake_req);
    UNSTAKING_QUEUE.save(deps.storage, &unstaking_queue)?;

    Ok(Response::default())
}

fn on_restake_reward(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.result {
        cosmwasm_std::SubMsgResult::Ok(_) => {
            let restaking_queue = RESTAKING_QUEUE.load(deps.storage)?;
            RESTAKING_QUEUE.remove(deps.storage);

            let res = execute_stake(
                ExecuteContext::new(
                    deps,
                    MessageInfo {
                        sender: restaking_queue.delegator,
                        funds: restaking_queue.accumulated_rewards,
                    },
                    env,
                ),
                Some(Addr::unchecked(restaking_queue.validator)),
            )?;
            Ok(res.add_attribute("action", "restake-reward"))
        }
        cosmwasm_std::SubMsgResult::Err(e) => {
            RESTAKING_QUEUE.remove(deps.storage);
            Err(ContractError::new(e.as_str()))
        }
    }
}
