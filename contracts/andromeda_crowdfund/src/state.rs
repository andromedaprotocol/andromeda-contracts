use common::ado_base::recipient::Recipient;
use cosmwasm_std::{Addr, Coin, SubMsg, Uint128};
use cw0::Expiration;
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The config.
pub const CONFIG: Item<Config> = Item::new("config");

/// Sale started if and only if STATE.may_load is Some and !duration.is_expired()
pub const STATE: Item<State> = Item::new("state");

/// Relates buyer address to vector of purchases.
pub const PURCHASES: Map<&str, Vec<Purchase>> = Map::new("buyers");

/// Maps token_id -> whether or not it has been purchased or not.
pub const TOKEN_AVAILABILITY: Map<&str, bool> = Map::new("token_availability");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Purchase {
    pub token_id: String,
    // amount of tax paid
    pub tax_amount: Uint128,
    // sub messages for rates sending
    pub msgs: Vec<SubMsg>,
    pub purchaser: String,
}

/// Can be updated if sale NOT ongoing.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub token_address: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub expiration: Expiration,
    pub price: Coin,
    pub min_tokens_sold: Uint128,
    pub max_amount_per_wallet: Uint128,
    pub amount_sold: Uint128,
    /// The amount of funds to send to recipient if sale successful. This already
    /// takes into account the royalties and taxes.
    pub amount_to_send: Uint128,
    pub recipient: Recipient,
}
