#![cfg(all(not(target_arch = "wasm32"), feature = "testing"))]

use crate::contract::{execute, instantiate, query};
use amp::kernel::{ExecuteMsg, InstantiateMsg, QueryMsg};
use cosmwasm_std::Empty;
use cw_multi_test::{Contract, ContractWrapper};

pub fn mock_andromeda_kernel() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new_with_empty(execute, instantiate, query);
    Box::new(contract)
}

pub fn mock_kernel_instantiate_message() -> InstantiateMsg {
    InstantiateMsg {}
}

pub fn mock_upsert_key_address(key: impl Into<String>, value: impl Into<String>) -> ExecuteMsg {
    ExecuteMsg::UpsertKeyAddress {
        key: key.into(),
        value: value.into(),
    }
}
