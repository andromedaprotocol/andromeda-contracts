use andromeda_math::time_gate::{ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::{ado_base::MigrateMsg, contract_interface, deploy::ADOMetadata};

pub const CONTRACT_ID: &str = "time-gate";

contract_interface!(TimeGateContract, CONTRACT_ID, "andromeda_time_gate.wasm");
