use andromeda_non_fungible_tokens::pow_cw721::{ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::{ado_base::MigrateMsg, contract_interface, deploy::ADOMetadata};

pub const CONTRACT_ID: &str = "pow-cw721";

contract_interface!(PowCw721Contract, CONTRACT_ID, "andromeda_pow_cw721.wasm");
