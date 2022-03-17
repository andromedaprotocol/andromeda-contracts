use common::ado_base::{modules::Module, recipient::Recipient, AndromedaMsg, AndromedaQuery};
use cosmwasm_std::{Coin, Uint128};
use cw0::Expiration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub token_address: String,
    pub modules: Option<Vec<Module>>,
    pub primitive_address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AndrReceive(AndromedaMsg),
    StartSale {
        /// When the sale ends.
        expiration: Expiration,
        /// The price per token.
        price: Coin,
        /// The minimum amount of tokens sold to go through with the sale.
        min_tokens_sold: Uint128,
        /// The amount of tokens a wallet can purchase, default is 1.
        max_amount_per_wallet: Option<Uint128>,
        /// The recipient of the funds if the sale met the minimum sold.
        recipient: Recipient,
    },
    Purchase {
        token_id: String,
    },
    /// Allow a user to claim their own refund if the minimum number of tokens are not sold.
    ClaimRefund {},
    EndSale {
        limit: Option<u32>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    AndrQuery(AndromedaQuery),
    State {},
    Config {},
}
