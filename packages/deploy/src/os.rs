use crate::error::DeployError;
use andromeda_std::ado_base::MigrateMsg;
use andromeda_std::amp::AndrAddr;
use andromeda_std::os::*;
use cw_orch::prelude::*;
use cw_orch_daemon::{DaemonBase, DaemonBuilder, TxSender, Wallet};
use kernel::{ExecuteMsgFns, QueryMsgFns};

use crate::chains::{get_chain, ANDROMEDA_TESTNET};
use crate::contracts::*;

struct OperatingSystemDeployment {
    daemon: DaemonBase<Wallet>,
    kernel: KernelContract<DaemonBase<Wallet>>,
    adodb: ADODBContract<DaemonBase<Wallet>>,
    vfs: VFSContract<DaemonBase<Wallet>>,
    economics: EconomicsContract<DaemonBase<Wallet>>,
    ibc_registry: IBCRegistryContract<DaemonBase<Wallet>>,
}

impl OperatingSystemDeployment {
    pub fn new(chain: ChainInfo) -> Self {
        let daemon = DaemonBuilder::new(chain).build().unwrap();
        let kernel = KernelContract::new(daemon.clone());
        let adodb = ADODBContract::new(daemon.clone());
        let vfs = VFSContract::new(daemon.clone());
        let economics = EconomicsContract::new(daemon.clone());
        let ibc_registry = IBCRegistryContract::new(daemon.clone());

        Self {
            daemon,
            kernel,
            adodb,
            vfs,
            economics,
            ibc_registry,
        }
    }

    pub fn upload(&self) -> Result<(), DeployError> {
        self.kernel.upload()?;
        self.adodb.upload()?;
        self.vfs.upload()?;
        self.economics.upload()?;
        self.ibc_registry.upload()?;
        Ok(())
    }

    /// Instantiates OS contracts
    ///
    /// If a kernel is provided we look to migrate the existing contracts instead of creating new ones.
    pub fn instantiate(&self, kernel_address: Option<String>) -> Result<(), DeployError> {
        let sender = self.daemon.sender().address();

        // If kernel address is provided, use it and migrate the contract to the new version
        if let Some(address) = kernel_address {
            let code_id = self.kernel.code_id().unwrap();
            self.kernel.set_address(&Addr::unchecked(address));
            self.kernel.migrate(&MigrateMsg {}, code_id)?;
        } else {
            let kernel_msg = kernel::InstantiateMsg {
                owner: Some(sender.to_string()),
                chain_name: ANDROMEDA_TESTNET.network_info.chain_name.to_string(),
            };
            self.kernel.instantiate(&kernel_msg, Some(&sender), None)?;
            println!("Kernel address: {}", self.kernel.address().unwrap());
        };

        // For each module we check if it's been instantiated already.
        // If it has, we migrate it to the new code id.
        // If it hasn't, we instantiate it.

        let adodb_addr = self.kernel.key_address("adodb").ok();
        if let Some(addr) = adodb_addr {
            let code_id = self.adodb.code_id().unwrap();
            self.adodb.set_address(&addr);
            self.adodb.migrate(&MigrateMsg {}, code_id)?;
        } else {
            let adodb_msg = adodb::InstantiateMsg {
                owner: Some(sender.to_string()),
                kernel_address: self.kernel.address().unwrap().to_string(),
            };
            self.adodb.instantiate(&adodb_msg, Some(&sender), None)?;
        }

        let vfs_addr = self.kernel.key_address("vfs").ok();
        if let Some(addr) = vfs_addr {
            let code_id = self.vfs.code_id().unwrap();
            self.vfs.set_address(&addr);
            self.vfs.migrate(&MigrateMsg {}, code_id)?;
        } else {
            let vfs_msg = vfs::InstantiateMsg {
                owner: Some(sender.to_string()),
                kernel_address: self.kernel.address().unwrap().to_string(),
            };
            self.vfs.instantiate(&vfs_msg, Some(&sender), None)?;
        }

        let economics_addr = self.kernel.key_address("economics").ok();
        if let Some(addr) = economics_addr {
            let code_id = self.economics.code_id().unwrap();
            self.economics.set_address(&addr);
            self.economics.migrate(&MigrateMsg {}, code_id)?;
        } else {
            let economics_msg = economics::InstantiateMsg {
                owner: Some(sender.to_string()),
                kernel_address: self.kernel.address().unwrap().to_string(),
            };
            self.economics
                .instantiate(&economics_msg, Some(&sender), None)?;
        }

        let ibc_registry_addr = self.kernel.key_address("ibc_registry").ok();
        if let Some(addr) = ibc_registry_addr {
            let code_id = self.ibc_registry.code_id().unwrap();
            self.ibc_registry.set_address(&addr);
            self.ibc_registry.migrate(&MigrateMsg {}, code_id)?;
        } else {
            let ibc_registry_msg = ibc_registry::InstantiateMsg {
                owner: Some(sender.to_string()),
                kernel_address: self.kernel.address().unwrap(),
                service_address: AndrAddr::from_string(sender.to_string()),
            };
            self.ibc_registry
                .instantiate(&ibc_registry_msg, Some(&sender), None)?;
        }
        Ok(())
    }

    fn register_modules(&self) -> Result<(), DeployError> {
        self.kernel
            .upsert_key_address("vfs", self.vfs.address().unwrap())?;
        self.kernel
            .upsert_key_address("adodb", self.adodb.address().unwrap())?;
        self.kernel
            .upsert_key_address("economics", self.economics.address().unwrap())?;
        self.kernel
            .upsert_key_address("ibc_registry", self.ibc_registry.address().unwrap())?;
        Ok(())
    }
}

pub fn deploy(chain: String, kernel_address: Option<String>) -> Result<String, DeployError> {
    env_logger::init();
    let chain = get_chain(chain);
    let os_deployment = OperatingSystemDeployment::new(chain);
    log::info!("Starting OS deployment process");

    log::info!("Uploading contracts");
    os_deployment.upload()?;

    log::info!("Instantiating contracts");
    os_deployment.instantiate(kernel_address)?;

    log::info!("Registering modules");
    os_deployment.register_modules()?;

    log::info!("OS deployment process completed");
    Ok(os_deployment.kernel.address().unwrap().to_string())
}