use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

use common::mission::AndrAddress;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG_KEY: &str = "config";
pub const CONFIG: Item<Config> = Item::new(CONFIG_KEY);

pub const STATE_KEY: &str = "state";
pub const STATE: Item<State> = Item::new(STATE_KEY);

pub const USER_INFO: Map<&Addr, UserInfo> = Map::new("users");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Auction Contract address to which MARS tokens can be deposited for bootstrapping MARS-UST Pool
    pub auction_contract_address: Option<AndrAddress>,
    /// Timestamp when Contract will start accepting deposits
    pub init_timestamp: u64,
    /// Deposit Window Length
    pub deposit_window: u64,
    /// Withdrawal Window Length
    pub withdrawal_window: u64,
    /// Total Token lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Uint128,
    /// The token being given as incentive.
    pub incentive_token: String,
    /// The native token being deposited.
    pub native_denom: String,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// Total NATIVE deposited at the end of Lockdrop window. This value remains unchanged post the lockdrop window
    pub total_native_locked: Uint128,
    /// Number of Tokens deposited into the bootstrap auction contract
    pub total_delegated: Uint128,
    /// Boolean value indicating if the user can withdraw thier MARS rewards or not
    pub are_claims_allowed: bool,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    /// Total UST amount deposited by the user across all his lockup positions
    pub total_native_locked: Uint128,
    /// TOKEN incentives deposited to the auction contract for TOKEN-UST Bootstrapping auction
    pub delegated_incentives: Uint128,
    /// Boolean value indicating if the lockdrop_rewards for the lockup positions have been claimed or not
    pub lockdrop_claimed: bool,
    pub withdrawal_flag: bool,
}
