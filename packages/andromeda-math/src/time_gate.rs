use andromeda_std::{
    amp::AndrAddr,
    andr_exec, andr_instantiate, andr_query,
    common::{expiration::Expiry, Milliseconds},
};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Addr;
use cw_utils::Expiration;

#[andr_instantiate]
#[cw_serde]
pub struct InstantiateMsg {
    pub gate_addresses: Vec<AndrAddr>,
    pub cycle_start_time: Option<Expiry>,
    pub time_interval: Option<u64>,
}

#[andr_exec]
#[cw_serde]
pub enum ExecuteMsg {
    UpdateCycleStartTime { cycle_start_time: Option<Expiry> },
    UpdateGateAddresses { new_gate_addresses: Vec<AndrAddr> },
    UpdateTimeInterval { time_interval: u64 },
}

#[andr_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Addr)]
    GetCurrentAdoPath {},
    #[returns((Expiration, Milliseconds))]
    GetCycleStartTime {},
    #[returns(Vec<AndrAddr>)]
    GetGateAddresses {},
    #[returns(String)]
    GetTimeInterval {},
}
