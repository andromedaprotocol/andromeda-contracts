use andromeda_protocol::auction::AuctionStateResponse;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map, U128Key};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub token_addr: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TokenAuctionState {
    pub start_time: u64,
    pub end_time: u64,
    pub high_bidder_addr: Addr,
    pub high_bidder_amount: Uint128,
    pub coin_denom: String,
    pub auction_id: Uint128,
    pub claimed: bool,
}

impl Into<AuctionStateResponse> for TokenAuctionState {
    fn into(self) -> AuctionStateResponse {
        AuctionStateResponse {
            start_time: self.start_time,
            end_time: self.end_time,
            high_bidder_addr: self.high_bidder_addr.to_string(),
            high_bidder_amount: self.high_bidder_amount,
            claimed: self.claimed,
            coin_denom: self.coin_denom,
            auction_id: self.auction_id,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Bid {
    pub bidder: String,
    pub amount: Uint128,
}

pub const NEXT_AUCTION_ID: Item<Uint128> = Item::new("next_auction_id");
pub const CONFIG: Item<Config> = Item::new("config");

pub const AUCTION_IDS: Map<&str, Vec<Uint128>> = Map::new("auction_ids"); // token_id -> [auction_ids]
pub const BIDS: Map<U128Key, Vec<Bid>> = Map::new("bids"); // auction_id -> [bids]

pub const TOKEN_AUCTION_STATE: Map<U128Key, TokenAuctionState> = Map::new("auction_token_state");
