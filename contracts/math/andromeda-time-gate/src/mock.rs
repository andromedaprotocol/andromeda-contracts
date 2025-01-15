#![cfg(all(not(target_arch = "wasm32"), feature = "testing"))]
use crate::contract::{execute, instantiate, query};
use andromeda_math::time_gate::CycleStartTime;
use andromeda_math::time_gate::{ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::amp::AndrAddr;
use andromeda_testing::mock::MockApp;
use andromeda_testing::{
    mock_ado,
    mock_contract::{ExecuteResult, MockADO, MockContract},
};
use cosmwasm_std::{Addr, Coin, Empty};
use cw_multi_test::{Contract, ContractWrapper, Executor};

pub struct MockTimeGate(Addr);
mock_ado!(MockTimeGate, ExecuteMsg, QueryMsg);

impl MockTimeGate {
    pub fn mock_instantiate(
        code_id: u64,
        sender: Addr,
        app: &mut MockApp,
        kernel_address: String,
        owner: Option<String>,
        cycle_start_time: CycleStartTime,
        time_interval: Option<u64>,
        gate_addresses: Vec<AndrAddr>,
    ) -> MockTimeGate {
        let msg = mock_time_gate_instantiate_msg(
            kernel_address,
            owner,
            gate_addresses,
            cycle_start_time,
            time_interval,
        );
        let addr = app.instantiate_contract(
            code_id,
            sender.clone(),
            &msg,
            &[],
            "Time Gate Contract",
            Some(sender.to_string()),
        )?;
        MockTimeGate(Addr::unchecked(addr))
    }

    pub fn mock_execute_update_cycle_start_time(
        &self,
        app: &mut MockApp,
        sender: Addr,
        cycle_start_time: CycleStartTime,
        funds: Option<Coin>,
    ) -> ExecuteResult {
        let msg = ExecuteMsg::UpdateCycleStartTime { cycle_start_time };
        if let Some(funds) = funds {
            app.execute_contract(sender, self.addr().clone(), &msg, &[funds])
        } else {
            app.execute_contract(sender, self.addr().clone(), &msg, &[])
        }
    }

    pub fn mock_execute_update_gate_addresses(
        &self,
        app: &mut MockApp,
        sender: Addr,
        gate_addresses: Vec<AndrAddr>,
        funds: Option<Coin>,
    ) -> ExecuteResult {
        let msg = ExecuteMsg::UpdateGateAddresses {
            new_gate_addresses: gate_addresses,
        };
        if let Some(funds) = funds {
            app.execute_contract(sender, self.addr().clone(), &msg, &[funds])
        } else {
            app.execute_contract(sender, self.addr().clone(), &msg, &[])
        }
    }

    pub fn mock_execute_update_time_interval(
        &self,
        app: &mut MockApp,
        sender: Addr,
        time_interval: Option<u64>,
        funds: Option<Coin>,
    ) -> ExecuteResult {
        let msg = ExecuteMsg::UpdateTimeInterval { time_interval };
        if let Some(funds) = funds {
            app.execute_contract(sender, self.addr().clone(), &msg, &[funds])
        } else {
            app.execute_contract(sender, self.addr().clone(), &msg, &[])
        }
    }

    pub fn mock_query_cycle_start_time(&self, app: &mut MockApp) -> CycleStartTime {
        let msg = QueryMsg::GetCycleStartTime {};
        let res: CycleStartTime = self.query(app, msg);
        res
    }

    pub fn mock_query_gate_addresses(&self, app: &mut MockApp) -> Vec<AndrAddr> {
        let msg = QueryMsg::GetGateAddresses {};
        let res: Vec<AndrAddr> = self.query(app, msg);
        res
    }

    pub fn mock_query_current_ado_path(&self, app: &mut MockApp) -> Addr {
        let msg = QueryMsg::GetCurrentAdoPath {};
        let res: Addr = self.query(app, msg);
        res
    }

    pub fn mock_query_time_interval(&self, app: &mut MockApp) -> String {
        let msg = QueryMsg::GetTimeInterval {};
        let res: String = self.query(app, msg);
        res
    }
}

pub fn mock_andromeda_time_gate() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new_with_empty(execute, instantiate, query);
    Box::new(contract)
}

pub fn mock_time_gate_instantiate_msg(
    kernel_address: String,
    owner: Option<String>,
    gate_addresses: Vec<AndrAddr>,
    cycle_start_time: CycleStartTime,
    time_interval: Option<u64>,
) -> InstantiateMsg {
    InstantiateMsg {
        kernel_address,
        owner,
        gate_addresses,
        cycle_start_time,
        time_interval,
    }
}
