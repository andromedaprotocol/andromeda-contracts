use common::{ado_base::recipient::Recipient, mission::AndrAddress};
use cosmwasm_std::{Coin, SubMsg, Uint128};
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

/// Contains token ids that have not been purchased.
pub const AVAILABLE_TOKENS: Map<&str, bool> = Map::new("available_tokens");

/// Is set to true when at least one sale has been conducted. This is used to disallow minting if
/// config.can_mint_after_sale is false.
pub const SALE_CONDUCTED: Item<bool> = Item::new("sale_conducted");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Purchase {
    /// The token id being purchased.
    pub token_id: String,
    /// Amount of tax paid.
    pub tax_amount: Uint128,
    /// sub messages for sending funds for rates.
    pub msgs: Vec<SubMsg>,
    /// The purchaser of the token.
    pub purchaser: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// The address of the token contract whose tokens are being sold.
    pub token_address: AndrAddress,
    /// Whether or not the owner can mint additional tokens after the sale has been conducted.
    pub can_mint_after_sale: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// The expiration denoting when the sale ends.
    pub expiration: Expiration,
    /// The price of each token.
    pub price: Coin,
    /// The minimum number of tokens sold for the sale to go through.
    pub min_tokens_sold: Uint128,
    /// The max number of tokens allowed per wallet.
    pub max_amount_per_wallet: Uint128,
    /// Number of tokens sold.
    pub amount_sold: Uint128,
    /// The amount of funds to send to recipient if sale successful. This already
    /// takes into account the royalties and taxes.
    pub amount_to_send: Uint128,
    /// Number of tokens transferred to purchasers if sale was successful.
    pub amount_transferred: Uint128,
    /// The recipient of the raised funds if the sale is successful.
    pub recipient: Recipient,
}
