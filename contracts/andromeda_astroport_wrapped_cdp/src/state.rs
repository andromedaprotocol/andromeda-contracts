use cosmwasm_std::Addr;
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub astroport_router_contract: Addr,
    pub astroport_staking_contract: Addr,
    pub astroport_vesting_contract: Addr,
    pub astroport_maker_contract: Addr,
}
