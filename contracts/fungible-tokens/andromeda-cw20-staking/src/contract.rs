use std::{ops::Mul, str::FromStr};

use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    ado_contract::ADOContract,
    common::{actions::call_action, context::ExecuteContext, encode_binary, Milliseconds},
    error::ContractError,
};
use cosmwasm_std::{
    attr, entry_point, Attribute, BlockInfo, Decimal, Decimal256, Order, QuerierWrapper, Reply,
    StdError, Uint256,
};
use cosmwasm_std::{
    ensure, from_json, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, Storage,
    Uint128,
};
use cw20::Cw20ReceiveMsg;
use cw_asset::{Asset, AssetInfo, AssetInfoUnchecked};

use crate::{
    allocated_rewards::update_allocated_index,
    state::{
        get_stakers, Staker, StakerRewardInfo, CONFIG, MAX_REWARD_TOKENS, REWARD_TOKENS, STAKERS,
        STAKER_REWARD_INFOS, STATE,
    },
};

use andromeda_fungible_tokens::cw20_staking::{
    Config, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, RewardToken, RewardTokenUnchecked,
    RewardType, StakerResponse, State,
};

use cw_utils::nonpayable;

// Version info, for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-cw20-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let additional_reward_tokens = if let Some(additional_rewards) = msg.additional_rewards {
        ensure!(
            additional_rewards.len() <= MAX_REWARD_TOKENS as usize,
            ContractError::MaxRewardTokensExceeded {
                max: MAX_REWARD_TOKENS,
            }
        );
        let staking_token = AssetInfoUnchecked::cw20(msg.staking_token.to_string().to_lowercase());
        let staking_token_identifier = msg.staking_token.to_string();
        let additional_rewards: Result<Vec<RewardToken>, ContractError> = additional_rewards
            .into_iter()
            .map(|r| {
                // Staking token cannot be used as an additional reward as it is a reward by
                // default.
                ensure!(
                    staking_token != r.asset_info,
                    ContractError::InvalidAsset {
                        asset: staking_token_identifier.to_string(),
                    }
                );
                r.check(&env.block, deps.api)
            })
            .collect();
        additional_rewards?
    } else {
        vec![]
    };
    for token in additional_reward_tokens.iter() {
        REWARD_TOKENS.save(deps.storage, &token.to_string(), token)?;
    }
    CONFIG.save(
        deps.storage,
        &Config {
            staking_token: msg.staking_token,
            number_of_reward_tokens: additional_reward_tokens.len() as u32,
        },
    )?;
    STATE.save(
        deps.storage,
        &State {
            total_share: Uint128::zero(),
        },
    )?;

    let contract = ADOContract::default();
    let resp = contract.instantiate(
        deps.storage,
        env,
        deps.api,
        &deps.querier,
        info.clone(),
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
    let _contract = ADOContract::default();
    let action_response = call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;
    let res = match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(ctx, msg),
        ExecuteMsg::AddRewardToken { reward_token } => execute_add_reward_token(ctx, reward_token),
        ExecuteMsg::RemoveRewardToken { reward_token } => {
            execute_remove_reward_token(ctx, reward_token)
        }
        ExecuteMsg::ReplaceRewardToken {
            origin_reward_token,
            reward_token,
        } => execute_replace_reward_token(ctx, origin_reward_token, reward_token),
        ExecuteMsg::UpdateGlobalIndexes { asset_infos } => match asset_infos {
            None => update_global_indexes(
                ctx.deps.storage,
                &ctx.env.block,
                &ctx.deps.querier,
                Milliseconds::from_seconds(ctx.env.block.time.seconds()),
                ctx.env.contract.address,
                None,
            ),
            Some(asset_infos) => {
                let asset_infos: Result<Vec<AssetInfo>, ContractError> = asset_infos
                    .iter()
                    .map(|a| Ok(a.check(ctx.deps.api, None)?))
                    .collect();
                update_global_indexes(
                    ctx.deps.storage,
                    &ctx.env.block,
                    &ctx.deps.querier,
                    Milliseconds::from_seconds(ctx.env.block.time.seconds()),
                    ctx.env.contract.address,
                    Some(asset_infos?),
                )
            }
        },
        ExecuteMsg::UnstakeTokens { amount } => execute_unstake_tokens(ctx, amount),
        ExecuteMsg::ClaimRewards {} => execute_claim_rewards(ctx),
        _ => ADOContract::default().execute(ctx, msg),
    }?;
    Ok(res
        .add_submessages(action_response.messages)
        .add_attributes(action_response.attributes)
        .add_events(action_response.events))
}

fn receive_cw20(ctx: ExecuteContext, msg: Cw20ReceiveMsg) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    ensure!(
        !msg.amount.is_zero(),
        ContractError::InvalidFunds {
            msg: "Amount must be non-zero".to_string(),
        }
    );

    match from_json(&msg.msg)? {
        Cw20HookMsg::StakeTokens {} => {
            execute_stake_tokens(deps, env, msg.sender, info.sender.to_string(), msg.amount)
        }
        Cw20HookMsg::UpdateGlobalIndex {} => update_global_indexes(
            deps.storage,
            &env.block,
            &deps.querier,
            Milliseconds::from_seconds(env.block.time.seconds()),
            env.contract.address,
            Some(vec![AssetInfo::cw20(info.sender)]),
        ),
    }
}

fn execute_add_reward_token(
    ctx: ExecuteContext,
    reward_token: RewardTokenUnchecked,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    let contract = ADOContract::default();
    ensure!(
        contract.is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    let mut config = CONFIG.load(deps.storage)?;

    let new_number = config.number_of_reward_tokens.checked_add(1);
    match new_number {
        Some(new_number) => config.number_of_reward_tokens = new_number,
        None => return Err(ContractError::Overflow {}),
    }
    ensure!(
        config.number_of_reward_tokens <= MAX_REWARD_TOKENS,
        ContractError::MaxRewardTokensExceeded {
            max: MAX_REWARD_TOKENS,
        }
    );
    let mut reward_token = reward_token.check(&env.block, deps.api)?;
    let reward_token_string = reward_token.to_string();

    let reward_token_option = REWARD_TOKENS.may_load(deps.storage, &reward_token_string)?;
    ensure!(
        reward_token_option.map_or(true, |reward_token| !reward_token.is_active),
        ContractError::InvalidAsset {
            asset: reward_token_string,
        }
    );

    let staking_token_address = config.staking_token.get_raw_address(&deps.as_ref())?;
    let staking_token = AssetInfo::cw20(deps.api.addr_validate(staking_token_address.as_str())?);
    ensure!(
        staking_token != reward_token.asset_info,
        ContractError::InvalidAsset {
            asset: reward_token.to_string(),
        }
    );

    let reward_token_string = reward_token.to_string();

    let state = STATE.load(deps.storage)?;
    update_global_index(
        &env.block,
        &deps.querier,
        Milliseconds::from_seconds(env.block.time.seconds()),
        env.contract.address,
        &state,
        &mut reward_token,
    )?;

    REWARD_TOKENS.save(deps.storage, &reward_token_string, &reward_token)?;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "add_reward_token")
        .add_attribute("added_token", reward_token_string))
}

fn execute_remove_reward_token(
    ctx: ExecuteContext,
    reward_token_string: String,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    let contract = ADOContract::default();
    ensure!(
        contract.is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    // Set reward token as inactive
    match REWARD_TOKENS.load(deps.storage, &reward_token_string) {
        Ok(mut reward_token) if reward_token.is_active => {
            // Need to save current status before setting reward token as inactive
            // This is important in case the reward token is allocated token
            let state = STATE.load(deps.storage)?;
            update_global_index(
                &env.block,
                &deps.querier,
                Milliseconds::from_seconds(env.block.time.seconds()),
                env.contract.address,
                &state,
                &mut reward_token,
            )?;

            reward_token.is_active = false;
            REWARD_TOKENS.save(deps.storage, &reward_token_string, &reward_token)?;
        }
        _ => {
            return Err(ContractError::InvalidAsset {
                asset: reward_token_string,
            })
        }
    }

    let mut config = CONFIG.load(deps.storage)?;
    config.number_of_reward_tokens -= 1;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "remove_reward_token")
        .add_attribute(
            "number_of_reward_tokens",
            config.number_of_reward_tokens.to_string(),
        )
        .add_attribute("removed_token", reward_token_string))
}

fn execute_replace_reward_token(
    ctx: ExecuteContext,
    origin_reward_token_string: String,
    reward_token: RewardTokenUnchecked,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    let contract = ADOContract::default();

    // Only owner can replace reward token
    ensure!(
        contract.is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    let config = CONFIG.load(deps.storage)?;

    // Validate token to be replaced
    let mut reward_token = reward_token.check(&env.block, deps.api)?;
    let reward_token_string = reward_token.to_string();
    ensure!(
        !REWARD_TOKENS.has(deps.storage, &reward_token_string),
        ContractError::InvalidAsset {
            asset: reward_token_string,
        }
    );

    ensure!(
        reward_token_string != origin_reward_token_string,
        ContractError::InvalidAsset {
            asset: reward_token_string,
        }
    );

    let staking_token_address = config.staking_token.get_raw_address(&deps.as_ref())?;
    let staking_token = AssetInfo::cw20(deps.api.addr_validate(staking_token_address.as_str())?);
    ensure!(
        staking_token != reward_token.asset_info,
        ContractError::InvalidAsset {
            asset: reward_token.to_string(),
        }
    );

    // Set original token as inactive
    match REWARD_TOKENS.load(deps.storage, &origin_reward_token_string) {
        Ok(mut origin_reward_token) if origin_reward_token.is_active => {
            // Need to save current status before setting reward token as inactive
            // This is important in case the reward token is allocated token
            let state = STATE.load(deps.storage)?;
            update_global_index(
                &env.block,
                &deps.querier,
                Milliseconds::from_seconds(env.block.time.seconds()),
                env.contract.address.clone(),
                &state,
                &mut origin_reward_token,
            )?;

            origin_reward_token.is_active = false;
            REWARD_TOKENS.save(
                deps.storage,
                &origin_reward_token_string,
                &origin_reward_token,
            )?;
        }
        _ => {
            return Err(ContractError::InvalidAsset {
                asset: origin_reward_token_string,
            })
        }
    }

    let state = STATE.load(deps.storage)?;
    update_global_index(
        &env.block,
        &deps.querier,
        Milliseconds::from_seconds(env.block.time.seconds()),
        env.contract.address,
        &state,
        &mut reward_token,
    )?;

    REWARD_TOKENS.save(deps.storage, &reward_token_string, &reward_token)?;

    Ok(Response::new()
        .add_attribute("action", "replace_reward_token")
        .add_attribute("origin_reward_token", origin_reward_token_string)
        .add_attribute("new_reward_token", reward_token.to_string()))
}
/// The foundation for this approach is inspired by Anchor's staking implementation:
/// https://github.com/Anchor-Protocol/anchor-token-contracts/blob/15c9d6f9753bd1948831f4e1b5d2389d3cf72c93/contracts/gov/src/staking.rs#L15
fn execute_stake_tokens(
    deps: DepsMut,
    env: Env,
    sender: String,
    token_address: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let staking_token_address = config.staking_token.get_raw_address(&deps.as_ref())?;
    ensure!(
        token_address == staking_token_address,
        ContractError::InvalidFunds {
            msg: "Deposited cw20 token is not the staking token".to_string(),
        }
    );

    let mut state = STATE.load(deps.storage)?;
    let mut staker = STAKERS.may_load(deps.storage, &sender)?.unwrap_or_default();

    // Update indexes, important for allocated rewards.
    update_global_indexes(
        deps.storage,
        &env.block,
        &deps.querier,
        Milliseconds::from_seconds(env.block.time.seconds()),
        env.contract.address.clone(),
        None,
    )?;
    // Update the rewards for the user. This must be done before the new share is calculated.
    update_staker_rewards(deps.storage, &sender, &staker)?;

    let staking_token = AssetInfo::cw20(deps.api.addr_validate(staking_token_address.as_ref())?);

    // Balance already increased, so subtract deposit amount
    let total_balance = staking_token
        .query_balance(&deps.querier, env.contract.address.to_string())?
        .checked_sub(amount)?;

    let share = if total_balance.is_zero() || state.total_share.is_zero() {
        amount
    } else {
        amount.multiply_ratio(state.total_share, total_balance)
    };

    staker.share = staker.share.checked_add(share)?;
    state.total_share = state.total_share.checked_add(share)?;

    STATE.save(deps.storage, &state)?;
    STAKERS.save(deps.storage, &sender, &staker)?;

    Ok(Response::new()
        .add_attribute("action", "stake_tokens")
        .add_attribute("sender", sender)
        .add_attribute("share", share)
        .add_attribute("amount", amount))
}

fn execute_unstake_tokens(
    ctx: ExecuteContext,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    nonpayable(&info)?;
    let sender = info.sender.as_str();

    let staking_token = get_staking_token(deps.as_ref())?;

    let total_balance = staking_token.query_balance(&deps.querier, env.contract.address.clone())?;

    let staker = STAKERS.may_load(deps.storage, sender)?;
    if let Some(mut staker) = staker {
        let mut state = STATE.load(deps.storage)?;
        // Update indexes, important for allocated rewards.
        update_global_indexes(
            deps.storage,
            &env.block,
            &deps.querier,
            Milliseconds::from_seconds(env.block.time.seconds()),
            env.contract.address,
            None,
        )?;
        update_staker_rewards(deps.storage, sender, &staker)?;

        let withdraw_share = amount
            .map(|v| {
                std::cmp::max(
                    v.multiply_ratio(state.total_share, total_balance),
                    Uint128::new(1),
                )
            })
            .unwrap_or(staker.share);

        ensure!(
            withdraw_share <= staker.share,
            ContractError::InvalidWithdrawal {
                msg: Some("Desired amount exceeds balance".to_string()),
            }
        );

        let withdraw_amount = amount
            .unwrap_or_else(|| withdraw_share.multiply_ratio(total_balance, state.total_share));

        let asset = Asset {
            info: staking_token,
            amount: withdraw_amount,
        };
        staker.share = staker.share.checked_sub(withdraw_share)?;
        state.total_share = state.total_share.checked_sub(withdraw_share)?;

        STATE.save(deps.storage, &state)?;
        STAKERS.save(deps.storage, sender, &staker)?;

        Ok(Response::new()
            .add_attribute("action", "unstake_tokens")
            .add_attribute("sender", sender)
            .add_attribute("withdraw_amount", withdraw_amount)
            .add_attribute("withdraw_share", withdraw_share)
            .add_message(asset.transfer_msg(info.sender)?))
    } else {
        Err(ContractError::WithdrawalIsEmpty {})
    }
}

fn execute_claim_rewards(ctx: ExecuteContext) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    let sender = info.sender.as_str();
    if let Some(staker) = STAKERS.may_load(deps.storage, sender)? {
        // Update indexes, important for allocated rewards.
        update_global_indexes(
            deps.storage,
            &env.block,
            &deps.querier,
            Milliseconds::from_seconds(env.block.time.seconds()),
            env.contract.address.clone(),
            None,
        )?;
        update_staker_rewards(deps.storage, sender, &staker)?;
        let mut msgs: Vec<CosmosMsg> = vec![];

        let reward_tokens = get_reward_tokens(deps.storage)?;

        for mut token in reward_tokens {
            let token_string = token.to_string();

            // Since we call `update_staker_rewards` first, this entry will always exist.
            let mut staker_reward_info =
                STAKER_REWARD_INFOS.load(deps.storage, (sender, &token_string))?;
            let rewards: Uint128 =
                Decimal::from_str(staker_reward_info.pending_rewards.to_string().as_str())?
                    * Uint128::from(1u128);

            let decimals: Decimal256 =
                staker_reward_info.pending_rewards - Decimal256::from_ratio(rewards, 1u128);

            if !rewards.is_zero() {
                // Reduce pending rewards for staker to what is left over after rounding.
                staker_reward_info.pending_rewards = decimals;

                STAKER_REWARD_INFOS.save(
                    deps.storage,
                    (sender, &token_string),
                    &staker_reward_info,
                )?;

                // Reduce reward balance if is non-allocated token.
                if let RewardType::NonAllocated {
                    previous_reward_balance,
                    ..
                } = &mut token.reward_type
                {
                    *previous_reward_balance = previous_reward_balance.checked_sub(rewards)?;
                    REWARD_TOKENS.save(deps.storage, &token_string, &token)?;
                }

                let asset = Asset {
                    info: AssetInfoUnchecked::from_str(&token_string)?.check(deps.api, None)?,
                    amount: rewards,
                };
                msgs.push(asset.transfer_msg(sender)?);
            }
            if !token.is_active {
                let reward_balance = token
                    .asset_info
                    .query_balance(&deps.querier, &env.contract.address)?;
                let reward_balance = Decimal256::from_str(&format!("{0}", reward_balance))?;
                let rewards_ceil = staker_reward_info
                    .index
                    .mul(Decimal256::from_str(&format!("{0}", staker.share))?)
                    .ceil();
                // if reward balance token is equal to the rewards, inactive reward token can be removed as it is all distributed
                if reward_balance == rewards_ceil {
                    REWARD_TOKENS.remove(deps.storage, &token_string);
                }
            }
        }

        ensure!(!msgs.is_empty(), ContractError::WithdrawalIsEmpty {});

        Ok(Response::new()
            .add_attribute("action", "claim_rewards")
            .add_messages(msgs))
    } else {
        Err(ContractError::WithdrawalIsEmpty {})
    }
}

fn update_global_indexes(
    storage: &mut dyn Storage,
    block_info: &BlockInfo,
    querier: &QuerierWrapper,
    current_timestamp: Milliseconds,
    contract_address: Addr,
    asset_infos: Option<Vec<AssetInfo>>,
) -> Result<Response, ContractError> {
    let state = STATE.load(storage)?;

    let all_assets = get_reward_tokens(storage)?
        .into_iter()
        .map(|r| r.asset_info)
        .collect();

    let asset_infos = asset_infos.unwrap_or(all_assets);

    let mut attributes: Vec<Attribute> = vec![attr("action", "update_global_indexes")];
    for asset_info in asset_infos {
        let asset_info_string = asset_info.to_string();
        let reward_token = REWARD_TOKENS.may_load(storage, &asset_info_string)?;
        match reward_token {
            None => {
                return Err(ContractError::InvalidAsset {
                    asset: asset_info_string.clone(),
                })
            }
            Some(mut reward_token) => {
                update_global_index(
                    block_info,
                    querier,
                    current_timestamp,
                    contract_address.clone(),
                    &state,
                    &mut reward_token,
                )?;
                REWARD_TOKENS.save(storage, &asset_info_string, &reward_token)?;

                attributes.push(attr(asset_info_string, reward_token.index.to_string()));
            }
        }
    }

    Ok(Response::new().add_attributes(attributes))
}

fn update_global_index(
    block_info: &BlockInfo,
    querier: &QuerierWrapper,
    current_timestamp: Milliseconds,
    contract_address: Addr,
    state: &State,
    reward_token: &mut RewardToken,
) -> Result<(), ContractError> {
    // In this case there is no point updating the index if no one is staked.
    if state.total_share.is_zero() {
        return Ok(());
    }

    match &reward_token.reward_type {
        RewardType::NonAllocated {
            previous_reward_balance,
            init_timestamp,
        } => {
            update_nonallocated_index(
                state,
                querier,
                reward_token,
                *previous_reward_balance,
                contract_address,
                current_timestamp,
                *init_timestamp,
            )?;
        }
        RewardType::Allocated {
            allocation_config,
            allocation_state,
            init_timestamp,
        } => {
            update_allocated_index(
                block_info,
                state.total_share,
                reward_token,
                allocation_config.clone(),
                allocation_state.clone(),
                current_timestamp,
                *init_timestamp,
            )?;
        }
    }

    Ok(())
}

/// This approach was inspired by Lido's bluna reward system.
/// https://github.com/lidofinance/lido-terra-contracts/tree/d7026b9142d718f9b5b6be03b1af33040499553c/contracts/lido_terra_reward/src
fn update_nonallocated_index(
    state: &State,
    querier: &QuerierWrapper,
    reward_token: &mut RewardToken,
    previous_reward_balance: Uint128,
    contract_address: Addr,
    curr_timestamp: Milliseconds,
    init_timestamp: Milliseconds,
) -> Result<(), ContractError> {
    if curr_timestamp < init_timestamp || !reward_token.is_active {
        return Ok(());
    }
    let reward_balance = reward_token
        .asset_info
        .query_balance(querier, contract_address)?;
    let deposited_amount = reward_balance.checked_sub(previous_reward_balance)?;

    reward_token.index += Decimal256::from_ratio(deposited_amount, state.total_share);

    reward_token.reward_type = RewardType::NonAllocated {
        previous_reward_balance: reward_balance,
        init_timestamp,
    };

    Ok(())
}

fn update_staker_rewards(
    storage: &mut dyn Storage,
    staker_address: &str,
    staker: &Staker,
) -> Result<(), ContractError> {
    let reward_tokens: Vec<RewardToken> = get_reward_tokens(storage)?;
    for token in reward_tokens {
        let token_string = token.to_string();
        let mut staker_reward_info = STAKER_REWARD_INFOS
            .may_load(storage, (staker_address, &token_string))?
            .unwrap_or_default();
        update_staker_reward_info(staker, &mut staker_reward_info, token);
        STAKER_REWARD_INFOS.save(
            storage,
            (staker_address, &token_string),
            &staker_reward_info,
        )?;
    }
    Ok(())
}

fn get_reward_tokens(storage: &dyn Storage) -> Result<Vec<RewardToken>, ContractError> {
    REWARD_TOKENS
        .range(storage, None, None, Order::Ascending)
        .map(|v| {
            let (_, reward_token) = v?;
            Ok(reward_token)
        })
        .collect()
}

fn update_staker_reward_info(
    staker: &Staker,
    staker_reward_info: &mut StakerRewardInfo,
    reward_token: RewardToken,
) {
    let staker_share = Uint256::from(staker.share);
    let rewards = (reward_token.index - staker_reward_info.index) * staker_share;

    staker_reward_info.index = reward_token.index;
    staker_reward_info.pending_rewards += Decimal256::from_ratio(rewards, 1u128);
}

pub(crate) fn get_staking_token(deps: Deps) -> Result<AssetInfo, ContractError> {
    let _contract = ADOContract::default();
    let config = CONFIG.load(deps.storage)?;

    // let mission_contract = contract.get_app_contract(deps.storage)?;
    let staking_token_address = config.staking_token.get_raw_address(&deps)?;

    let staking_token = AssetInfo::cw20(deps.api.addr_validate(staking_token_address.as_ref())?);

    Ok(staking_token)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Config {} => encode_binary(&query_config(deps)?),
        QueryMsg::State {} => encode_binary(&query_state(deps)?),
        QueryMsg::Staker { address } => encode_binary(&query_staker(deps, env, address)?),
        QueryMsg::Stakers { start_after, limit } => {
            encode_binary(&query_stakers(deps, env, start_after, limit)?)
        }
        _ => ADOContract::default().query(deps, env, msg),
    }
}

fn query_config(deps: Deps) -> Result<Config, ContractError> {
    Ok(CONFIG.load(deps.storage)?)
}

fn query_state(deps: Deps) -> Result<State, ContractError> {
    Ok(STATE.load(deps.storage)?)
}

fn query_staker(deps: Deps, env: Env, address: String) -> Result<StakerResponse, ContractError> {
    let staker = STAKERS.load(deps.storage, &address)?;
    let state = STATE.load(deps.storage)?;
    let pending_rewards =
        get_pending_rewards(deps.storage, &deps.querier, &env, &address, &staker)?;
    let staking_token = get_staking_token(deps)?;
    let total_balance = staking_token.query_balance(&deps.querier, env.contract.address)?;
    let balance = staker
        .share
        .multiply_ratio(total_balance, state.total_share);
    Ok(StakerResponse {
        address,
        share: staker.share,
        pending_rewards,
        balance,
    })
}

/// Gets the pending rewards for the user in the form of a vector of (token, pending_reward)
/// tuples.
pub(crate) fn get_pending_rewards(
    storage: &dyn Storage,
    querier: &QuerierWrapper,
    env: &Env,
    address: &str,
    staker: &Staker,
) -> Result<Vec<(String, Uint128)>, ContractError> {
    let reward_tokens: Vec<RewardToken> = get_reward_tokens(storage)?;
    let mut pending_rewards = vec![];
    let state = STATE.load(storage)?;
    let current_timestamp = Milliseconds::from_seconds(env.block.time.seconds());
    for mut token in reward_tokens {
        let token_string = token.to_string();
        let mut staker_reward_info = STAKER_REWARD_INFOS
            .may_load(storage, (address, &token_string))?
            .unwrap_or_default();
        update_global_index(
            &env.block,
            querier,
            current_timestamp,
            env.contract.address.to_owned(),
            &state,
            &mut token,
        )?;
        update_staker_reward_info(staker, &mut staker_reward_info, token);
        pending_rewards.push((
            token_string,
            Decimal::from_str(staker_reward_info.pending_rewards.to_string().as_str())?
                * Uint128::from(1u128),
        ))
    }
    Ok(pending_rewards)
}

fn query_stakers(
    deps: Deps,
    env: Env,
    start_after: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<StakerResponse>, ContractError> {
    let start = start_after.as_deref();
    get_stakers(deps, &deps.querier, deps.api, &env, start, limit)
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
