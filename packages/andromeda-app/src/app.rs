use common::ado_base::{AndromedaMsg, AndromedaQuery};
use cosmwasm_std::Binary;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct AppComponent {
    pub name: String,
    pub ado_type: String,
    pub instantiate_msg: Binary,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    pub app: Vec<AppComponent>,
    pub name: String,
    pub primitive_contract: String,
    // Used for automation
    pub target_ados: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AndrReceive(AndromedaMsg),
    AddAppComponent { component: AppComponent },
    ClaimOwnership { name: Option<String> },
    Fire {},
    ProxyMessage { name: String, msg: Binary },
    UpdateAddress { name: String, addr: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    AndrQuery(AndromedaQuery),
    GetAddress { name: String },
    GetComponents {},
    ComponentExists { name: String },
    GetAddressesWithName {},
    GetAddresses {},
    Config {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ComponentAddress {
    pub name: String,
    pub address: String,
}

#[cfg(test)]
mod tests {
    // use super::*;
}
