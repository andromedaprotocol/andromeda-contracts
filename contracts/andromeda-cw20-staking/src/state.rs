use cosmwasm_bignumber::Decimal256;
use cosmwasm_std::{Env, Order, QuerierWrapper, Storage, Uint128};
use cw_storage_plus::{Bound, Item, Map};

use crate::contract::get_pending_rewards;
use andromeda_protocol::cw20_staking::{RewardToken, StakerResponse};
use common::{error::ContractError, mission::AndrAddress};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const STAKERS: Map<&str, Staker> = Map::new("stakers");

/// Maps asset -> reward_info
pub const REWARD_TOKENS: Map<&str, RewardToken> = Map::new("reward_tokens");

/// Maps (staker, asset) -> reward_info
pub const STAKER_REWARD_INFOS: Map<(&str, &str), StakerRewardInfo> =
    Map::new("staker_reward_infos");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// The token accepted for staking.
    pub staking_token: AndrAddress,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// The total share of the staking token in the contract.
    pub total_share: Uint128,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Staker {
    /// Total staked share.
    pub share: Uint128,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerRewardInfo {
    /// The index of this particular reward.
    pub index: Decimal256,
    /// The pending rewards for this particular reward.
    pub pending_rewards: Decimal256,
}

const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;
pub(crate) fn get_stakers(
    storage: &dyn Storage,
    querier: &QuerierWrapper,
    env: &Env,
    start_after: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<StakerResponse>, ContractError> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    STAKERS
        .range(storage, start, None, Order::Ascending)
        .take(limit)
        .map(|elem| {
            let (k, v) = elem?;
            let address: String = String::from_utf8(k)?;
            let pending_rewards = get_pending_rewards(storage, querier, env, &address, &v)?;
            Ok(StakerResponse {
                address,
                share: v.share,
                pending_rewards,
            })
        })
        .collect()
}
