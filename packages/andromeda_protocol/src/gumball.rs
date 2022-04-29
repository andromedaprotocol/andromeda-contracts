use common::{
    ado_base::{recipient::Recipient, AndromedaMsg, AndromedaQuery},
    mission::AndrAddress,
};
use cosmwasm_std::{Binary, Coin, Uint128};
use cw721_base::MintMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::cw721::TokenExtension;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub andromeda_cw721_contract: AndrAddress,
    pub randomness_source: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AndrReceive(AndromedaMsg),
    Mint(Box<MintMsg<TokenExtension>>),
    Buy {},
    /// Sets price, max amount per wallet, and recipient
    SetSaleDetails {
        /// The price per token.
        price: Coin,
        /// The amount of tokens a wallet can purchase, default is 1.
        max_amount_per_wallet: Option<Uint128>,
        /// The recipient of the funds if the sale met the minimum sold.
        recipient: Recipient,
    },
    /// Automatically switches to opposite status.
    /// True means buying is allowed and minting is halted. False means the opposite.
    SwitchStatus {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    AndrQuery(AndromedaQuery),
    NumberOfNFTs {},
    SaleDetails {},
    Status {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RandQueryMsg {
    LatestDrand {},
    GetRandomness { round: u64 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct NumberOfNFTsResponse {
    pub number: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct StatusResponse {
    pub status: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LatestRandomResponse {
    pub round: u64,
    pub randomness: Binary,
    pub worker: String,
}
