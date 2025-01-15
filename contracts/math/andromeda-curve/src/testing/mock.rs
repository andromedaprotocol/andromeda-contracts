use andromeda_math::curve::{
    CurveConfig, ExecuteMsg, GetCurveConfigResponse, GetPlotYFromXResponse, InstantiateMsg,
    QueryMsg,
};
use andromeda_std::{
    amp::AndrAddr,
    error::ContractError,
    testing::mock_querier::{mock_dependencies_custom, WasmMockQuerier, MOCK_KERNEL_CONTRACT},
};
use cosmwasm_std::{
    from_json,
    testing::{mock_env, mock_info, MockApi, MockStorage},
    Decimal, Deps, DepsMut, MessageInfo, OwnedDeps, Response,
};

use crate::contract::{execute, instantiate, query};

pub type MockDeps = OwnedDeps<MockStorage, MockApi, WasmMockQuerier>;

pub fn proper_initialization(
    curve_config: CurveConfig,
    authorized_operator_addresses: Option<Vec<AndrAddr>>,
) -> (MockDeps, MessageInfo) {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    let msg = InstantiateMsg {
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
        curve_config,
        authorized_operator_addresses,
    };
    let env = mock_env();
    let res = instantiate(deps.as_mut(), env, info.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());
    (deps, info)
}

pub fn error_initialization(
    curve_config: CurveConfig,
    authorized_operator_addresses: Option<Vec<AndrAddr>>,
) -> ContractError {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);
    let msg = InstantiateMsg {
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
        curve_config,
        authorized_operator_addresses,
    };
    let env = mock_env();
    let res = instantiate(deps.as_mut(), env, info.clone(), msg).unwrap_err();
    res
}

pub fn update_curve_config(
    deps: DepsMut<'_>,
    curve_config: CurveConfig,
    sender: &str,
) -> Result<Response, ContractError> {
    let msg = ExecuteMsg::UpdateCurveConfig { curve_config };
    let info = mock_info(sender, &[]);
    execute(deps, mock_env(), info, msg)
}

pub fn reset(deps: DepsMut<'_>, sender: &str) -> Result<Response, ContractError> {
    let msg = ExecuteMsg::Reset {};
    let info = mock_info(sender, &[]);
    execute(deps, mock_env(), info, msg)
}

pub fn query_curve_config(deps: Deps) -> Result<GetCurveConfigResponse, ContractError> {
    let res = query(deps, mock_env(), QueryMsg::GetCurveConfig {});
    match res {
        Ok(res) => Ok(from_json(res).unwrap()),
        Err(err) => Err(err),
    }
}

pub fn query_plot_y_from_x(
    deps: Deps,
    x_value: Decimal,
) -> Result<GetPlotYFromXResponse, ContractError> {
    let res = query(deps, mock_env(), QueryMsg::GetPlotYFromX { x_value });
    match res {
        Ok(res) => Ok(from_json(res).unwrap()),
        Err(err) => Err(err),
    }
}
