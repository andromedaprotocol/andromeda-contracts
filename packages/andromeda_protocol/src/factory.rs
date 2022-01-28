use crate::modules::ModuleDefinition;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Create new token
    Create {
        name: String,
        symbol: String,
        modules: Vec<ModuleDefinition>,
    },
    UpdateCodeId {
        code_id_key: String,
        code_id: u64,
    },
    /// Update token contract address by symbol
    UpdateAddress {
        symbol: String,
        new_address: String,
    },
    /// Update current contract owner
    UpdateOwner {
        address: String,
    },
    UpdateOperator {
        operators: Vec<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Query token contract address by its symbol
    GetAddress {
        symbol: String,
    },
    /// All code IDs for Andromeda contracts
    CodeId {
        key: String,
    },
    /// The current contract owner
    ContractOwner {},
    IsOperator {
        address: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AddressResponse {
    pub address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CodeIdResponse {
    pub code_id: u64,
}
