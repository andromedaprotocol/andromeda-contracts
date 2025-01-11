use std::time::Instant;

use cosmwasm_std::Addr;
use cw_multi_test::{App, ContractWrapper, Executor};
use andromeda_std::{
    amp::{ADO_DB_KEY, VFS_KEY, AndrAddr},
    os::{
        kernel::{ExecuteMsg as KernelExecuteMsg, InstantiateMsg as KernelInstantiateMsg},
        adodb::InstantiateMsg as AdodbInstantiateMsg,
        vfs::InstantiateMsg as VfsInstantiateMsg,
        economics::InstantiateMsg as economics_InstantiateMsg,
        ibc_registry::InstantiateMsg as IbcRegistryInstantiateMsg,
    },
};
use andromeda_fungible_tokens::cw20::InstantiateMsg as Cw20InstantiateMsg;

// This struct will hold our test environment, similar to MockAndromeda but for multitest
pub struct MultitestAndromeda {
    // The App represents our blockchain environment
    pub app: App,
    // Contract addresses for core components
    pub kernel_address: String,
    pub adodb_address: String,
    pub vfs_address: String,
    pub cw20_address: String,
    pub economics_address: String,
    pub ibc_registry_address: String,
}

impl MultitestAndromeda {
    pub fn new() -> Self {
        // Create new blockchain environment
        let mut app = App::default();
        
        // First, store the contract code for each component
        let kernel_code_id = app.store_code(Box::new(ContractWrapper::new(
            andromeda_kernel::contract::execute,
            andromeda_kernel::contract::instantiate,
            andromeda_kernel::contract::query,
        ).with_reply(andromeda_kernel::contract::reply)));

        let adodb_code_id = app.store_code(Box::new(ContractWrapper::new(
            andromeda_adodb::contract::execute,
            andromeda_adodb::contract::instantiate,
            andromeda_adodb::contract::query,
        )));

        let vfs_code_id = app.store_code(Box::new(ContractWrapper::new(
            andromeda_vfs::contract::execute,
            andromeda_vfs::contract::instantiate,
            andromeda_vfs::contract::query,
        )));

        let cw20_code_id = app.store_code(Box::new(ContractWrapper::new(
            andromeda_cw20::contract::execute,
            andromeda_cw20::contract::instantiate,
            andromeda_cw20::contract::query,
        )));

        let economics_code_id = app.store_code(Box::new(ContractWrapper::new(
            andromeda_economics::contract::execute,
            andromeda_economics::contract::instantiate,
            andromeda_economics::contract::query,
        )));

        let ibc_registry_code_id = app.store_code(Box::new(ContractWrapper::new(
            andromeda_ibc_registry::contract::execute,
            andromeda_ibc_registry::contract::instantiate,
            andromeda_ibc_registry::contract::query,
        )));

        // Now instantiate the kernel first
        let kernel_address = app
            .instantiate_contract(
                kernel_code_id,
                Addr::unchecked("owner"),
                &KernelInstantiateMsg {
                    chain_name: "test-chain".to_string(),
                    owner: None,
                },
                &[],
                "Kernel",
                None,
            )
            .unwrap()
            .to_string();

        // Then instantiate ADODB with kernel address
        let adodb_address = app
            .instantiate_contract(
                adodb_code_id,
                Addr::unchecked("owner"),
                &AdodbInstantiateMsg {
                    kernel_address: kernel_address.clone(),
                    owner: None,
                },
                &[],
                "ADODB",
                None,
            )
            .unwrap()
            .to_string();

        // Then VFS
        let vfs_address = app
            .instantiate_contract(
                vfs_code_id,
                Addr::unchecked("owner"),
                &VfsInstantiateMsg {
                    kernel_address: kernel_address.clone(),
                    owner: None,
                },
                &[],
                "VFS",
                None,
            )
            .unwrap()
            .to_string();

        let cw20_address: String = app.instantiate_contract(
            cw20_code_id,
            Addr::unchecked("owner"),
            &Cw20InstantiateMsg {
                marketing: None,
                kernel_address: kernel_address.clone(),
                owner: Some("owner".to_string()),
                name: "Test Token".to_string(),
                symbol: "TEST".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: None,
            },
            &[],
            "Test Token",
            None,
        ).unwrap().to_string();

        let economics_address = app
            .instantiate_contract(
                economics_code_id,
                Addr::unchecked("owner"),
                &economics_InstantiateMsg {
                    kernel_address: kernel_address.clone(),
                    owner: None,
                },
                &[],
                "Economics Engine",
                None,
            )
            .unwrap()
            .to_string();

        let ibc_registry_address = app
            .instantiate_contract(
                ibc_registry_code_id,
                Addr::unchecked("owner"),
                &IbcRegistryInstantiateMsg {
                    kernel_address: Addr::unchecked(kernel_address.clone()),
                    owner: None,
                    service_address: AndrAddr::from_string("service_address".to_string()),
                },
                &[],
                "IBC Registry",
                None,
            )
            .unwrap()
            .to_string();

        // Register addresses in kernel
        app.execute_contract(
            Addr::unchecked("owner"),
            Addr::unchecked(&kernel_address),
            &KernelExecuteMsg::UpsertKeyAddress {
                key: ADO_DB_KEY.to_string(),
                value: adodb_address.clone(),
            },
            &[],
        ).unwrap();

        app.execute_contract(
            Addr::unchecked("owner"),
            Addr::unchecked(&kernel_address),
            &KernelExecuteMsg::UpsertKeyAddress {
                key: ADO_DB_KEY.to_string(),
                value: cw20_address.clone(),
            },
            &[],
        ).unwrap();

        app.execute_contract(
            Addr::unchecked("owner"),
            Addr::unchecked(&kernel_address),
            &KernelExecuteMsg::UpsertKeyAddress {
                key: VFS_KEY.to_string(),
                value: vfs_address.clone(),
            },
            &[],
        ).unwrap();

        MultitestAndromeda {
            app,
            kernel_address,
            adodb_address,
            vfs_address,
            cw20_address,
            economics_address,
            ibc_registry_address,
        }
    }

    // Helper function for IBC testing setup
    pub fn setup_ibc_test() -> (Self, Self) {
        // Create two instances representing different chains
        let chain_a = Self::new();
        let chain_b = Self::new();
        
        // Here we could set up IBC channels between the chains
        // This part would need to be implemented based on specific IBC testing needs
        
        (chain_a, chain_b)
    }

    pub fn mock_users() -> Vec<Addr> {
        vec![
            Addr::unchecked("C1"),
            Addr::unchecked("A1"),
            Addr::unchecked("D1"),
        ]
    }
}
