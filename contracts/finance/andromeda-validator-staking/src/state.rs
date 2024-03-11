use andromeda_finance::validator_staking::Unstaking;
use cw_storage_plus::{Deque, Item};

use cosmwasm_std::Addr;

pub const DEFAULT_VALIDATOR: Item<Addr> = Item::new("default_validator");

pub const UNSTAKING_QUEUE: Deque<Unstaking> = Deque::new("unstaking_queue");
