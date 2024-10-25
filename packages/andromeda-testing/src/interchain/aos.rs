use andromeda_adodb::{mock::mock_adodb_instantiate_msg, ADODBContract};
use andromeda_economics::{mock::mock_economics_instantiate_msg, EconomicsContract};
use andromeda_kernel::{
    mock::{mock_kernel_instantiate_message, mock_upsert_key_address},
    KernelContract,
};
use andromeda_std::{
    amp::{ADO_DB_KEY, ECONOMICS_KEY, VFS_KEY},
    os,
};
use andromeda_vfs::{mock::mock_vfs_instantiate_message, VFSContract};
use cw_orch::{mock::MockBase, prelude::*};

pub struct InterchainAOS {
    pub kernel: KernelContract<MockBase>,
    pub economics: EconomicsContract<MockBase>,
    pub adodb: ADODBContract<MockBase>,
    pub vfs: VFSContract<MockBase>,
}

impl InterchainAOS {
    pub fn new(chain: MockBase, chain_name: String) -> Self {
        let kernel = KernelContract::new(chain.clone());
        let economics = EconomicsContract::new(chain.clone());
        let adodb = ADODBContract::new(chain.clone());
        let vfs = VFSContract::new(chain.clone());

        kernel.upload().unwrap();
        economics.upload().unwrap();
        adodb.upload().unwrap();
        vfs.upload().unwrap();

        let aos = Self {
            kernel,
            economics,
            adodb,
            vfs,
        };

        aos.instantiate(chain_name);
        aos.register_modules();
        aos
    }

    fn instantiate(&self, chain_name: String) {
        let init_msg = mock_kernel_instantiate_message(None, chain_name);
        self.kernel.instantiate(&init_msg, None, None).unwrap();
        let vfs_init_msg =
            mock_vfs_instantiate_message(self.kernel.address().unwrap().into_string(), None);
        self.vfs.instantiate(&vfs_init_msg, None, None).unwrap();
        let adodb_init_msg =
            mock_adodb_instantiate_msg(self.kernel.address().unwrap().into_string(), None);
        self.adodb.instantiate(&adodb_init_msg, None, None).unwrap();
        let economics_init_msg =
            mock_economics_instantiate_msg(self.kernel.address().unwrap().into_string(), None);
        self.economics
            .instantiate(&economics_init_msg, None, None)
            .unwrap();
    }

    fn register_modules(&self) {
        let msg = mock_upsert_key_address(VFS_KEY, self.vfs.address().unwrap().into_string());
        self.kernel.execute(&msg, None).unwrap();
        let msg = mock_upsert_key_address(ADO_DB_KEY, self.adodb.address().unwrap().into_string());
        self.kernel.execute(&msg, None).unwrap();
        let msg = mock_upsert_key_address(
            ECONOMICS_KEY,
            self.economics.address().unwrap().into_string(),
        );
        self.kernel.execute(&msg, None).unwrap();
    }

    pub fn assign_channels(&self, channel_id: String, foreign_chain_name: String) {
        let msg = os::kernel::ExecuteMsg::AssignChannels {
            ics20_channel_id: Some(String::from("transfer")),
            direct_channel_id: Some(channel_id),
            chain: foreign_chain_name,
            kernel_address: self.kernel.address().unwrap().into_string(),
        };

        self.kernel.execute(&msg, None).unwrap();
    }
}
