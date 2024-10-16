use andromeda_std::{andr_exec, andr_instantiate, andr_query};
use cosmwasm_schema::{cw_serde, QueryResponses};

#[andr_instantiate]
#[cw_serde]
pub struct InstantiateMsg {
    pub schema_json_string: String,
}

#[andr_exec]
#[cw_serde]
pub enum ExecuteMsg {
    UpdateSchema { new_schema_json_string: String },
}

#[andr_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ValidateDataResponse)]
    ValidateData { data: String },
}

#[cw_serde]
pub struct ValidateDataResponse {
    pub is_valid: bool,
}