use cosmwasm_std::{Decimal, Order, Storage, Uint128};
use cw_asset::AssetInfo;
use cw_storage_plus::{Bound, Item, Map};

use crate::contract::get_pending_rewards;
use andromeda_fungible_tokens::cw20_staking::StakerResponse;
use common::{app::AndrAddress, error::ContractError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const STAKERS: Map<&str, Staker> = Map::new("stakers");

/// Maps asset -> reward_info
pub const GLOBAL_REWARD_INFOS: Map<&str, GlobalRewardInfo> = Map::new("global_reward_infos");

/// Maps (staker, asset) -> reward_info
pub const STAKER_REWARD_INFOS: Map<(&str, &str), StakerRewardInfo> =
    Map::new("staker_reward_infos");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// The token accepted for staking.
    pub staking_token: AndrAddress,
    /// Any additional tokens used for rewards. Cannot include the staking token.
    pub additional_reward_tokens: Vec<AssetInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// The total share of the staking token in the contract.
    pub total_share: Uint128,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GlobalRewardInfo {
    /// The index of this particular reward.
    pub index: Decimal,
    /// The reward balance to compare to when updating the index.
    pub previous_reward_balance: Uint128,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Staker {
    /// Total staked share.
    pub share: Uint128,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerRewardInfo {
    /// The index of this particular reward.
    pub index: Decimal,
    /// The pending rewards for this particular reward.
    pub pending_rewards: Decimal,
}

const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;
pub(crate) fn get_stakers(
    storage: &dyn Storage,
    start_after: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<StakerResponse>, ContractError> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_deref().map(Bound::exclusive);

    STAKERS
        .range(storage, start, None, Order::Ascending)
        .take(limit)
        .map(|elem| {
            let (address, v) = elem?;
            let pending_rewards = get_pending_rewards(storage, &address, &v)?;
            Ok(StakerResponse {
                address,
                share: v.share,
                pending_rewards,
            })
        })
        .collect()
}
