use cw_multi_test::{App, ContractWrapper, Executor};
use cosmwasm_std::{Addr, IbcMsg, IbcTimeout, Response, Coin, BankMsg};
use andromeda_std::amp::{ADO_DB_KEY, VFS_KEY};
use andromeda_std::os::kernel::{ExecuteMsg as KernelExecuteMsg, InstantiateMsg as KernelInstantiateMsg};

pub enum ChainType {
    Andromeda,
    Basic,
}

pub struct ChainApp {
    pub app: App,
    pub chain_type: ChainType,
    pub chain_name: String,
    pub kernel_address: Option<Addr>,
    pub adodb_address: Option<Addr>,
    pub vfs_address: Option<Addr>,
    pub channels: Vec<(String, String)>, // (channel_id, counterparty_chain)
}

impl ChainApp {
    pub fn new(chain_type: ChainType, chain_name: &str) -> Self {
        let mut app = App::default();
        match chain_type {
            ChainType::Andromeda => Self::setup_andromeda(app, chain_name),
            ChainType::Basic => Self::setup_basic_chain(app, chain_name),
        }
    }

    fn setup_andromeda(mut app: App, chain_name: &str) -> Self {
        // Store core contract codes
        let kernel_code_id = app.store_code(Box::new(ContractWrapper::new(
            andromeda_kernel::contract::execute,
            andromeda_kernel::contract::instantiate,
            andromeda_kernel::contract::query,
        ).with_reply(andromeda_kernel::contract::reply)
         .with_ibc(
            andromeda_kernel::ibc::ibc_channel_connect,
            andromeda_kernel::ibc::ibc_channel_close,
            andromeda_kernel::ibc::ibc_packet_receive,
            andromeda_kernel::ibc::ibc_packet_ack,
            andromeda_kernel::ibc::ibc_packet_timeout,
        )));

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

        // Initialize contracts
        let kernel_address = app
            .instantiate_contract(
                kernel_code_id,
                Addr::unchecked("owner"),
                &KernelInstantiateMsg {
                    chain_name: chain_name.to_string(),
                    owner: None,
                },
                &[],
                "kernel",
                None,
            )
            .unwrap();

        let adodb_address = app
            .instantiate_contract(
                adodb_code_id,
                Addr::unchecked("owner"),
                &andromeda_std::os::adodb::InstantiateMsg {
                    kernel_address: kernel_address.to_string(),
                    owner: None,
                },
                &[],
                "adodb",
                None,
            )
            .unwrap();

        let vfs_address = app
            .instantiate_contract(
                vfs_code_id,
                Addr::unchecked("owner"),
                &andromeda_std::os::vfs::InstantiateMsg {
                    kernel_address: kernel_address.to_string(),
                    owner: None,
                },
                &[],
                "vfs",
                None,
            )
            .unwrap();

        // Register core addresses in kernel
        app.execute_contract(
            Addr::unchecked("owner"),
            kernel_address.clone(),
            &KernelExecuteMsg::UpsertKeyAddress {
                key: ADO_DB_KEY.to_string(),
                value: adodb_address.to_string(),
            },
            &[],
        )
        .unwrap();

        app.execute_contract(
            Addr::unchecked("owner"),
            kernel_address.clone(),
            &KernelExecuteMsg::UpsertKeyAddress {
                key: VFS_KEY.to_string(),
                value: vfs_address.to_string(),
            },
            &[],
        )
        .unwrap();

        Self {
            app,
            chain_type: ChainType::Andromeda,
            chain_name: chain_name.to_string(),
            kernel_address: Some(kernel_address),
            adodb_address: Some(adodb_address),
            vfs_address: Some(vfs_address),
            channels: vec![],
        }
    }

    fn setup_basic_chain(app: App, chain_name: &str) -> Self {
        Self {
            app,
            chain_type: ChainType::Basic,
            chain_name: chain_name.to_string(),
            kernel_address: None,
            adodb_address: None,
            vfs_address: None,
            channels: vec![],
        }
    }

    pub fn mint_tokens(&mut self, address: &str, coins: Vec<Coin>) {
        self.app.init_modules(|router, _, storage| {
            router
                .bank
                .init_balance(storage, &Addr::unchecked(address), coins)
                .unwrap()
        });
    }
}