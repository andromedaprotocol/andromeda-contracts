#![cfg(all(not(target_arch = "wasm32"), feature = "testing"))]

use crate::contract::{execute, instantiate, query, reply};
use andromeda_systems::current_block::{InstantiateMsg, QueryMsg};
use cosmwasm_std::Empty;
use cw_multi_test::{Contract, ContractWrapper};

pub fn mock_andromeda_system_current_block() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new_with_empty(execute, instantiate, query).with_reply(reply);
    Box::new(contract)
}

pub fn mock_andromeda_system_current_block_instantiate_message(
    kernel_address: impl Into<String>,
    owner: Option<String>,
    root: String,
    name: String,
) -> InstantiateMsg {
    InstantiateMsg {
        kernel_address: kernel_address.into(),
        owner,
        root,
        name,
    }
}

pub fn mock_query_current_block(path: impl Into<String>) -> QueryMsg {
    QueryMsg::GetCurrentBlockHeight {}
}
