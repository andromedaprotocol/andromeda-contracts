use andromeda_std::{amp::AndrAddr, andr_exec, andr_instantiate, andr_query, error::ContractError};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{ensure, Decimal};

#[andr_instantiate]
#[cw_serde]
pub struct InstantiateMsg {
    pub curve_config: CurveConfig,
    pub authorized_operator_addresses: Option<Vec<AndrAddr>>,
}

#[cw_serde]
pub enum CurveType {
    Growth,
    Decay,
}

#[cw_serde]
pub enum CurveConfig {
    ExpConfig {
        curve_type: CurveType,
        base_value: u64,
        multiple_variable_value: Option<u64>,
        constant_value: Option<u64>,
    },
}

impl CurveConfig {
    pub fn validate(&self) -> Result<(), ContractError> {
        match self {
            CurveConfig::ExpConfig {
                curve_type: _,
                base_value,
                multiple_variable_value: _,
                constant_value: _,
            } => {
                ensure!(
                    *base_value != 0,
                    ContractError::CustomError {
                        msg: "Base Value must be bigger than Zero".to_string()
                    }
                );
            }
        }
        Ok(())
    }
}

#[andr_exec]
#[cw_serde]
pub enum ExecuteMsg {
    UpdateCurveConfig { curve_config: CurveConfig },
    Reset {},
}

#[andr_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(GetCurveConfigResponse)]
    GetCurveConfig {},
    #[returns(GetPlotYFromXResponse)]
    GetPlotYFromX { x_value: Decimal },
}

#[cw_serde]
pub struct GetCurveConfigResponse {
    pub curve_config: CurveConfig,
}

#[cw_serde]
pub struct GetPlotYFromXResponse {
    pub y_value: String,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_validate_valid() {
        let curve_config = CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 4,
            multiple_variable_value: None,
            constant_value: None,
        };
        assert!(curve_config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid() {
        let curve_config = CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 0,
            multiple_variable_value: None,
            constant_value: None,
        };
        assert!(curve_config.validate().is_err());
    }
}
