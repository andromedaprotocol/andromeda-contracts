#![cfg(all(not(target_arch = "wasm32"), feature = "testing"))]
use crate::contract::{execute, instantiate, query};
use andromeda_math::curve::{
    CurveConfig, ExecuteMsg, GetCurveConfigResponse, GetPlotYFromXResponse, InstantiateMsg,
    QueryMsg,
};
use andromeda_std::amp::AndrAddr;
use andromeda_testing::mock::MockApp;
use andromeda_testing::{
    mock_ado,
    mock_contract::{ExecuteResult, MockADO, MockContract},
};
use cosmwasm_std::{Addr, Coin, Empty};
use cw_multi_test::{Contract, ContractWrapper, Executor};

pub struct MockCurve(Addr);
mock_ado!(MockCurve, ExecuteMsg, QueryMsg);

impl MockCurve {
    pub fn instantiate(
        code_id: u64,
        sender: Addr,
        app: &mut MockApp,
        kernel_address: String,
        owner: Option<String>,
        curve_config: CurveConfig,
        authorized_operator_addresses: Option<Vec<AndrAddr>>,
    ) -> MockCurve {
        let msg = mock_curve_instantiate_msg(
            kernel_address,
            owner,
            curve_config,
            authorized_operator_addresses,
        );
        let addr = app
            .instantiate_contract(
                code_id,
                sender.clone(),
                &msg,
                &[],
                "Curve Contract",
                Some(sender.to_string()),
            )
            .unwrap();
        MockCurve(Addr::unchecked(addr))
    }

    pub fn execute_update_curve_config(
        &self,
        app: &mut MockApp,
        sender: Addr,
        curve_config: CurveConfig,
        funds: Option<Coin>,
    ) -> ExecuteResult {
        let msg = ExecuteMsg::UpdateCurveConfig { curve_config };
        if let Some(funds) = funds {
            app.execute_contract(sender, self.addr().clone(), &msg, &[funds])
        } else {
            app.execute_contract(sender, self.addr().clone(), &msg, &[])
        }
    }

    pub fn execute_reset(
        &self,
        app: &mut MockApp,
        sender: Addr,
        funds: Option<Coin>,
    ) -> ExecuteResult {
        let msg = ExecuteMsg::Reset {};
        if let Some(funds) = funds {
            app.execute_contract(sender, self.addr().clone(), &msg, &[funds])
        } else {
            app.execute_contract(sender, self.addr().clone(), &msg, &[])
        }
    }

    pub fn query_config(&self, app: &mut MockApp) -> GetCurveConfigResponse {
        let msg = QueryMsg::GetCurveConfig {};
        let res: GetCurveConfigResponse = self.query(app, msg);
        res
    }

    pub fn query_plot_y_from_x(&self, app: &mut MockApp, x_value: u64) -> GetPlotYFromXResponse {
        let msg = QueryMsg::GetPlotYFromX { x_value };
        let res: GetPlotYFromXResponse = self.query(app, msg);
        res
    }
}

pub fn mock_andromeda_curve() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new_with_empty(execute, instantiate, query);
    Box::new(contract)
}

pub fn mock_curve_instantiate_msg(
    kernel_address: String,
    owner: Option<String>,
    curve_config: CurveConfig,
    authorized_operator_addresses: Option<Vec<AndrAddr>>,
) -> InstantiateMsg {
    InstantiateMsg {
        kernel_address,
        owner,
        curve_config,
        authorized_operator_addresses,
    }
}
