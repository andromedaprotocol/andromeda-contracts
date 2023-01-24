#![cfg(all(not(target_arch = "wasm32"), feature = "testing"))]

use crate::contract::{execute, instantiate, query, reply};
use andromeda_os::vfs::{ExecuteMsg, InstantiateMsg, QueryMsg};
use cosmwasm_std::{Addr, Empty};
use cw_multi_test::{Contract, ContractWrapper};

pub fn mock_andromeda_vfs() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new_with_empty(execute, instantiate, query).with_reply(reply);
    Box::new(contract)
}

pub fn mock_vfs_instantiate_message(kernel_address: impl Into<String>) -> InstantiateMsg {
    InstantiateMsg {
        kernel_address: kernel_address.into(),
    }
}

pub fn mock_register_user(username: impl Into<String>, address: Option<Addr>) -> ExecuteMsg {
    ExecuteMsg::RegisterUser {
        username: username.into(),
        address,
    }
}

pub fn mock_add_path(name: impl Into<String>, address: Addr) -> ExecuteMsg {
    ExecuteMsg::AddPath {
        name: name.into(),
        address,
    }
}

pub fn mock_resolve_path_query(path: impl Into<String>) -> QueryMsg {
    QueryMsg::ResolvePath { path: path.into() }
}
