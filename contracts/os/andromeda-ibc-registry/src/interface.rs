use andromeda_std::{
    ado_base::MigrateMsg, contract_interface, deploy::ADOMetadata, os::ibc_registry::*,
};

pub const CONTRACT_ID: &str = "ibc_registry";

contract_interface!(
    IBCRegistryContract,
    CONTRACT_ID,
    "andromeda_ibc_registry.wasm"
);
