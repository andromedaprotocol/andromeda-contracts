use andromeda_std::{amp::AndrAddr, andr_exec, andr_instantiate, andr_query, error::ContractError};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Coin, DepsMut, Timestamp, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const RESTAKING_ACTION: &str = "restake";

#[andr_instantiate]
#[cw_serde]
pub struct InstantiateMsg {
    pub default_validator: Addr,
}

#[andr_exec]
#[cw_serde]
#[cfg_attr(not(target_arch = "wasm32"), derive(cw_orch::ExecuteFns))]
pub enum ExecuteMsg {
    #[cfg_attr(not(target_arch = "wasm32"), cw_orch(payable))]
    Stake { validator: Option<Addr> },
    #[attrs(restricted)]
    Unstake {
        validator: Option<Addr>,
        amount: Option<Uint128>,
    },
    #[attrs(restricted)]
    Redelegate {
        src_validator: Option<Addr>,
        dst_validator: Addr,
        amount: Option<Uint128>,
    },
    Claim {
        validator: Option<Addr>,
        /// Defaults to false
        restake: Option<bool>,
    },
    #[attrs(restricted)]
    WithdrawFunds {
        denom: Option<String>,
        recipient: Option<AndrAddr>,
    },
    #[attrs(restricted)]
    UpdateDefaultValidator { validator: Addr },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UnstakingTokens {
    pub fund: Coin,
    pub payout_at: Timestamp,
}

#[andr_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Option<::cosmwasm_std::FullDelegation>)]
    StakedTokens { validator: Option<Addr> },

    #[returns(Option<Vec<UnstakingTokens>>)]
    UnstakedTokens {},

    #[returns(GetDefaultValidatorResponse)]
    DefaultValidator {},
}

impl InstantiateMsg {
    pub fn validate(&self, deps: &DepsMut) -> Result<bool, ContractError> {
        is_validator(deps, &self.default_validator)
    }
}

pub fn is_validator(deps: &DepsMut, validator: &Addr) -> Result<bool, ContractError> {
    let validator = deps.querier.query_validator(validator)?;
    if validator.is_none() {
        return Err(ContractError::InvalidValidator {});
    }
    Ok(true)
}

#[cw_serde]
pub struct GetDefaultValidatorResponse {
    pub default_validator: Addr,
}
