use andromeda_std::{amp::AndrAddr, common::Milliseconds};
use cw_storage_plus::Item;
use cw_utils::Expiration;

pub const GATE_ADDRESSES: Item<Vec<AndrAddr>> = Item::new("gate_addresses");
pub const CYCLE_START_TIME: Item<(Expiration, Milliseconds)> = Item::new("cycle_start_time");
pub const TIME_INTERVAL: Item<u64> = Item::new("time_interval");
