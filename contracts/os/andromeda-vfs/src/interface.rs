use andromeda_std::{ado_base::MigrateMsg, contract_interface, deploy::ADOMetadata, os::vfs::*};

pub const CONTRACT_ID: &str = "vfs";

contract_interface!(VFSContract, CONTRACT_ID, "andromeda_vfs.wasm");
