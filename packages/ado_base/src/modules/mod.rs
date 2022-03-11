use std::convert::TryInto;

use crate::state::ADOContract;
use cosmwasm_std::{Api, DepsMut, MessageInfo, Order, QuerierWrapper, Response, Storage, Uint64};
use cw_storage_plus::Bound;

use common::{
    ado_base::modules::{ADOType, InstantiateType, Module, ModuleInfoWithAddress},
    error::ContractError,
    require,
};

pub mod hooks;

impl<'a> ADOContract<'a> {
    /// A wrapper for `fn register_module`. The parameters are "extracted" from `DepsMut` to be able to
    /// execute this in a loop without cloning.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_register_module(
        &self,
        querier: &QuerierWrapper,
        storage: &mut dyn Storage,
        api: &dyn Api,
        sender: &str,
        module: &Module,
        ado_type: ADOType,
        should_validate: bool,
    ) -> Result<Response, ContractError> {
        require(
            self.is_contract_owner(storage, sender)? || self.is_operator(storage, sender),
            ContractError::Unauthorized {},
        )?;
        let mut resp = Response::default();
        let idx = self.register_module(storage, api, module)?;
        if let Some(inst_msg) = module.generate_instantiate_msg(storage, *querier, idx)? {
            resp = resp.add_submessage(inst_msg);
        }
        if should_validate {
            self.validate_modules(&self.load_modules(storage)?, ado_type)?;
        }
        Ok(resp.add_attribute("action", "register_module"))
    }

    /// A wrapper for `fn alter_module`.
    pub fn execute_alter_module(
        &self,
        deps: DepsMut,
        info: MessageInfo,
        module_idx: Uint64,
        module: &Module,
        ado_type: ADOType,
    ) -> Result<Response, ContractError> {
        let addr = info.sender.as_str();
        require(
            self.is_contract_owner(deps.storage, addr)? || self.is_operator(deps.storage, addr),
            ContractError::Unauthorized {},
        )?;
        let mut resp = Response::default();
        self.alter_module(deps.storage, deps.api, module_idx, module)?;
        if let Some(inst_msg) =
            module.generate_instantiate_msg(deps.storage, deps.querier, module_idx.u64())?
        {
            resp = resp.add_submessage(inst_msg);
        }
        self.validate_modules(&self.load_modules(deps.storage)?, ado_type)?;
        Ok(resp
            .add_attribute("action", "alter_module")
            .add_attribute("module_idx", module_idx))
    }

    /// A wrapper for `fn deregister_module`.
    pub fn execute_deregister_module(
        &self,
        deps: DepsMut,
        info: MessageInfo,
        module_idx: Uint64,
    ) -> Result<Response, ContractError> {
        let addr = info.sender.as_str();
        require(
            self.is_contract_owner(deps.storage, addr)? || self.is_operator(deps.storage, addr),
            ContractError::Unauthorized {},
        )?;
        self.deregister_module(deps.storage, module_idx)?;
        Ok(Response::default()
            .add_attribute("action", "deregister_module")
            .add_attribute("module_idx", module_idx))
    }

    /// Registers a module
    /// If the module has provided an address as its form of instantiation this address is recorded
    /// Each module is assigned a u64 index so as it can be unregistered/altered
    /// The assigned u64 index is used as the message id for use in the `reply` entry point of the contract
    fn register_module(
        &self,
        storage: &mut dyn Storage,
        api: &dyn Api,
        module: &Module,
    ) -> Result<u64, ContractError> {
        let idx = self.module_idx.may_load(storage)?.unwrap_or(1);
        let idx_str = idx.to_string();
        self.module_info.save(storage, &idx_str, module)?;
        self.module_idx.save(storage, &(idx + 1))?;
        if let InstantiateType::Address(addr) = &module.instantiate {
            self.module_addr
                .save(storage, &idx_str, &api.addr_validate(addr)?)?;
        }

        Ok(idx)
    }

    /// Deregisters a module.
    fn deregister_module(
        &self,
        storage: &mut dyn Storage,
        idx: Uint64,
    ) -> Result<(), ContractError> {
        let idx_str = idx.to_string();
        self.check_module_mutability(storage, &idx_str)?;
        self.module_info.remove(storage, &idx_str);
        self.module_addr.remove(storage, &idx_str);

        Ok(())
    }

    /// Alters a module
    /// If the module has provided an address as its form of instantiation this address is recorded
    /// Each module is assigned a u64 index so as it can be unregistered/altered
    /// The assigned u64 index is used as the message id for use in the `reply` entry point of the contract
    fn alter_module(
        &self,
        storage: &mut dyn Storage,
        api: &dyn Api,
        idx: Uint64,
        module: &Module,
    ) -> Result<(), ContractError> {
        let idx_str = idx.to_string();
        self.check_module_mutability(storage, &idx_str)?;
        self.module_info.save(storage, &idx_str, module)?;
        if let InstantiateType::Address(addr) = &module.instantiate {
            self.module_addr
                .save(storage, &idx_str, &api.addr_validate(addr)?)?;
        }
        Ok(())
    }

    fn check_module_mutability(
        &self,
        storage: &dyn Storage,
        idx_str: &str,
    ) -> Result<(), ContractError> {
        let existing_module = self.module_info.may_load(storage, idx_str)?;
        match existing_module {
            None => return Err(ContractError::ModuleDoesNotExist {}),
            Some(m) => {
                if !m.is_mutable {
                    return Err(ContractError::ModuleImmutable {});
                }
            }
        }
        Ok(())
    }

    /// Loads all registered modules in Vector form
    fn load_modules(&self, storage: &dyn Storage) -> Result<Vec<Module>, ContractError> {
        let module_idx = self.module_idx.may_load(storage)?.unwrap_or(1);
        let min = Some(Bound::Inclusive(1u64.to_le_bytes().to_vec()));
        // let max = Some(Bound::Inclusive(1u64.to_le_bytes().to_vec()));
        let modules: Vec<Module> = self
            .module_info
            .range(storage, min, None, Order::Ascending)
            .take(module_idx.try_into().unwrap())
            .flatten()
            .map(|(_vec, module)| module)
            .collect();

        Ok(modules)
    }

    /// Loads all registered module addresses in Vector form
    fn load_module_addresses(&self, storage: &dyn Storage) -> Result<Vec<String>, ContractError> {
        let module_idx = self.module_idx.may_load(storage)?.unwrap_or(1);
        let min = Some(Bound::Inclusive(1u64.to_le_bytes().to_vec()));
        // let max = Some(Bound::Inclusive(1u64.to_le_bytes().to_vec()));
        let module_addresses: Vec<String> = self
            .module_addr
            .range(storage, min, None, Order::Ascending)
            .take(module_idx.try_into().unwrap())
            .flatten()
            .map(|(_vec, addr)| addr.to_string())
            .collect();

        Ok(module_addresses)
    }

    /// Loads all modules with their registered addresses in Vector form
    fn load_modules_with_address(
        &self,
        storage: &dyn Storage,
    ) -> Result<Vec<ModuleInfoWithAddress>, ContractError> {
        let modules = self.load_modules(storage)?;
        let module_idx = self.module_idx.may_load(storage)?.unwrap_or(1);
        let min = Some(Bound::Inclusive(1u64.to_le_bytes().to_vec()));
        // let max = Some(Bound::Inclusive(1u64.to_le_bytes().to_vec()));
        let module_addresses: Vec<String> = self
            .module_addr
            .range(storage, min, None, Order::Ascending)
            .take(module_idx.try_into().unwrap())
            .flatten()
            .map(|(_vec, addr)| addr.to_string())
            .collect();

        let mut modules_with_addresses: Vec<ModuleInfoWithAddress> = Vec::new();
        for (index, module_address) in module_addresses.iter().enumerate() {
            let module_opt = modules.get(index);
            if let Some(module) = module_opt {
                modules_with_addresses.push(ModuleInfoWithAddress {
                    module: module.clone(),
                    address: module_address.to_string(),
                });
            }
        }

        Ok(modules_with_addresses)
    }

    /// Validates all modules.
    pub fn validate_modules(
        &self,
        modules: &[Module],
        ado_type: ADOType,
    ) -> Result<(), ContractError> {
        for module in modules {
            module.validate(modules, &ado_type)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::ado_base::modules::{ADDRESS_LIST, AUCTION, RECEIPT};
    use cosmwasm_std::{
        testing::{mock_dependencies, mock_info},
        Addr,
    };

    #[test]
    fn test_execute_register_module_unauthorized() {
        let mut deps = mock_dependencies(&[]);

        let module = Module {
            module_type: ADDRESS_LIST.to_owned(),
            instantiate: InstantiateType::Address("address".to_string()),
            is_mutable: false,
        };
        let deps_mut = deps.as_mut();
        ADOContract::default()
            .owner
            .save(deps_mut.storage, &Addr::unchecked("owner"))
            .unwrap();

        let res = ADOContract::default().execute_register_module(
            &deps_mut.querier,
            deps_mut.storage,
            deps_mut.api,
            "sender",
            &module,
            ADOType::CW20,
            true,
        );

        assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
    }

    #[test]
    fn test_execute_register_module_addr() {
        let mut deps = mock_dependencies(&[]);

        let module = Module {
            module_type: ADDRESS_LIST.to_owned(),
            instantiate: InstantiateType::Address("address".to_string()),
            is_mutable: false,
        };
        let deps_mut = deps.as_mut();
        ADOContract::default()
            .owner
            .save(deps_mut.storage, &Addr::unchecked("owner"))
            .unwrap();

        let res = ADOContract::default()
            .execute_register_module(
                &deps_mut.querier,
                deps_mut.storage,
                deps_mut.api,
                "owner",
                &module,
                ADOType::CW20,
                true,
            )
            .unwrap();

        assert_eq!(
            Response::default().add_attribute("action", "register_module"),
            res
        );

        assert_eq!(
            module,
            ADOContract::default()
                .module_info
                .load(deps.as_mut().storage, "1")
                .unwrap()
        );

        assert_eq!(
            "address".to_string(),
            ADOContract::default()
                .module_addr
                .load(deps.as_mut().storage, "1")
                .unwrap()
        );
    }

    #[test]
    fn test_execute_register_module_validate() {
        let mut deps = mock_dependencies(&[]);

        let module = Module {
            module_type: AUCTION.to_owned(),
            instantiate: InstantiateType::Address("address".to_string()),
            is_mutable: false,
        };
        let deps_mut = deps.as_mut();
        ADOContract::default()
            .owner
            .save(deps_mut.storage, &Addr::unchecked("owner"))
            .unwrap();

        let res = ADOContract::default().execute_register_module(
            &deps_mut.querier,
            deps_mut.storage,
            deps_mut.api,
            "owner",
            &module,
            ADOType::CW20,
            true,
        );

        assert_eq!(
            ContractError::IncompatibleModules {
                msg: "An Auction module cannot be used for a CW20 ADO".to_string()
            },
            res.unwrap_err(),
        );

        let res = ADOContract::default()
            .execute_register_module(
                &deps_mut.querier,
                deps_mut.storage,
                deps_mut.api,
                "owner",
                &module,
                ADOType::CW20,
                false,
            )
            .unwrap();

        assert_eq!(
            Response::default().add_attribute("action", "register_module"),
            res
        );
    }

    #[test]
    fn test_execute_alter_module_unauthorized() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("sender", &[]);
        let module = Module {
            module_type: ADDRESS_LIST.to_owned(),
            instantiate: InstantiateType::Address("address".to_string()),
            is_mutable: true,
        };
        ADOContract::default()
            .owner
            .save(deps.as_mut().storage, &Addr::unchecked("owner"))
            .unwrap();

        let res = ADOContract::default().execute_alter_module(
            deps.as_mut(),
            info,
            1u64.into(),
            &module,
            ADOType::CW20,
        );

        assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
    }

    #[test]
    fn test_execute_alter_module_addr() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("owner", &[]);
        let module = Module {
            module_type: ADDRESS_LIST.to_owned(),
            instantiate: InstantiateType::Address("address".to_string()),
            is_mutable: true,
        };

        ADOContract::default()
            .owner
            .save(deps.as_mut().storage, &Addr::unchecked("owner"))
            .unwrap();

        ADOContract::default()
            .module_info
            .save(deps.as_mut().storage, "1", &module)
            .unwrap();
        ADOContract::default()
            .module_addr
            .save(deps.as_mut().storage, "1", &Addr::unchecked("address"))
            .unwrap();

        let module = Module {
            module_type: RECEIPT.to_owned(),
            instantiate: InstantiateType::Address("other_address".to_string()),
            is_mutable: true,
        };

        let res = ADOContract::default()
            .execute_alter_module(deps.as_mut(), info, 1u64.into(), &module, ADOType::CW20)
            .unwrap();

        assert_eq!(
            Response::default()
                .add_attribute("action", "alter_module")
                .add_attribute("module_idx", "1"),
            res
        );

        assert_eq!(
            module,
            ADOContract::default()
                .module_info
                .load(deps.as_mut().storage, "1")
                .unwrap()
        );

        assert_eq!(
            "other_address".to_string(),
            ADOContract::default()
                .module_addr
                .load(deps.as_mut().storage, "1")
                .unwrap()
        );
    }

    #[test]
    fn test_execute_alter_module_immutable() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("owner", &[]);
        let module = Module {
            module_type: ADDRESS_LIST.to_owned(),
            instantiate: InstantiateType::Address("address".to_string()),
            is_mutable: false,
        };

        ADOContract::default()
            .owner
            .save(deps.as_mut().storage, &Addr::unchecked("owner"))
            .unwrap();

        ADOContract::default()
            .module_info
            .save(deps.as_mut().storage, "1", &module)
            .unwrap();
        ADOContract::default()
            .module_addr
            .save(deps.as_mut().storage, "1", &Addr::unchecked("address"))
            .unwrap();

        let module = Module {
            module_type: RECEIPT.to_owned(),
            instantiate: InstantiateType::Address("other_address".to_string()),
            is_mutable: true,
        };

        let res = ADOContract::default().execute_alter_module(
            deps.as_mut(),
            info,
            1u64.into(),
            &module,
            ADOType::CW20,
        );

        assert_eq!(ContractError::ModuleImmutable {}, res.unwrap_err());
    }

    #[test]
    fn test_execute_alter_module_nonexisting_module() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("owner", &[]);
        let module = Module {
            module_type: AUCTION.to_owned(),
            instantiate: InstantiateType::Address("address".to_string()),
            is_mutable: true,
        };

        ADOContract::default()
            .owner
            .save(deps.as_mut().storage, &Addr::unchecked("owner"))
            .unwrap();

        let res = ADOContract::default().execute_alter_module(
            deps.as_mut(),
            info,
            1u64.into(),
            &module,
            ADOType::CW20,
        );

        assert_eq!(ContractError::ModuleDoesNotExist {}, res.unwrap_err());
    }

    #[test]
    fn test_execute_alter_module_incompatible_module() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("owner", &[]);
        let module = Module {
            module_type: AUCTION.to_owned(),
            instantiate: InstantiateType::Address("address".to_string()),
            is_mutable: true,
        };

        ADOContract::default()
            .owner
            .save(deps.as_mut().storage, &Addr::unchecked("owner"))
            .unwrap();

        ADOContract::default()
            .module_info
            .save(deps.as_mut().storage, "1", &module)
            .unwrap();
        ADOContract::default()
            .module_addr
            .save(deps.as_mut().storage, "1", &Addr::unchecked("address"))
            .unwrap();

        let res = ADOContract::default().execute_alter_module(
            deps.as_mut(),
            info,
            1u64.into(),
            &module,
            ADOType::CW20,
        );

        assert_eq!(
            ContractError::IncompatibleModules {
                msg: "An Auction module cannot be used for a CW20 ADO".to_string()
            },
            res.unwrap_err(),
        );
    }

    #[test]
    fn test_execute_deregister_module_unauthorized() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("sender", &[]);
        ADOContract::default()
            .owner
            .save(deps.as_mut().storage, &Addr::unchecked("owner"))
            .unwrap();

        let res =
            ADOContract::default().execute_deregister_module(deps.as_mut(), info, 1u64.into());

        assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
    }

    #[test]
    fn test_execute_deregister_module() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("owner", &[]);
        ADOContract::default()
            .owner
            .save(deps.as_mut().storage, &Addr::unchecked("owner"))
            .unwrap();

        let module = Module {
            module_type: ADDRESS_LIST.to_owned(),
            instantiate: InstantiateType::Address("address".to_string()),
            is_mutable: true,
        };

        ADOContract::default()
            .module_info
            .save(deps.as_mut().storage, "1", &module)
            .unwrap();

        ADOContract::default()
            .module_addr
            .save(deps.as_mut().storage, "1", &Addr::unchecked("address"))
            .unwrap();

        let res = ADOContract::default()
            .execute_deregister_module(deps.as_mut(), info, 1u64.into())
            .unwrap();

        assert_eq!(
            Response::default()
                .add_attribute("action", "deregister_module")
                .add_attribute("module_idx", "1"),
            res
        );

        assert!(!ADOContract::default()
            .module_addr
            .has(deps.as_mut().storage, "1"));
        assert!(!ADOContract::default()
            .module_info
            .has(deps.as_mut().storage, "1"));
    }

    #[test]
    fn test_execute_deregister_module_immutable() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("owner", &[]);
        ADOContract::default()
            .owner
            .save(deps.as_mut().storage, &Addr::unchecked("owner"))
            .unwrap();

        let module = Module {
            module_type: ADDRESS_LIST.to_owned(),
            instantiate: InstantiateType::Address("address".to_string()),
            is_mutable: false,
        };

        ADOContract::default()
            .module_info
            .save(deps.as_mut().storage, "1", &module)
            .unwrap();

        ADOContract::default()
            .module_addr
            .save(deps.as_mut().storage, "1", &Addr::unchecked("address"))
            .unwrap();

        let res =
            ADOContract::default().execute_deregister_module(deps.as_mut(), info, 1u64.into());
        assert_eq!(ContractError::ModuleImmutable {}, res.unwrap_err());
    }

    #[test]
    fn test_execute_deregister_module_nonexisting_module() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("owner", &[]);
        ADOContract::default()
            .owner
            .save(deps.as_mut().storage, &Addr::unchecked("owner"))
            .unwrap();

        let res =
            ADOContract::default().execute_deregister_module(deps.as_mut(), info, 1u64.into());

        assert_eq!(ContractError::ModuleDoesNotExist {}, res.unwrap_err());
    }
}
