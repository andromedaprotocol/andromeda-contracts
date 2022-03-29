#[cfg(feature = "modules")]
use common::ado_base::modules::Module;
use common::{ado_base::QueryMsg, error::ContractError, parse_message};
use cosmwasm_std::{Addr, Binary, Storage};
use cw_storage_plus::{Item, Map};
#[cfg(feature = "withdraw")]
use terraswap::asset::AssetInfo;

//TODO: Make as many of these as possible pub(crate)
pub struct ADOContract<'a> {
    pub owner: Item<'a, Addr>,
    pub operators: Map<'a, &'a str, bool>,
    pub ado_type: Item<'a, String>,
    pub(crate) mission_contract: Item<'a, Addr>,
    #[cfg(feature = "primitive")]
    pub primitive_contract: Item<'a, Addr>,
    #[cfg(feature = "primitive")]
    pub(crate) cached_addresses: Map<'a, &'a str, String>,
    #[cfg(feature = "modules")]
    pub module_info: Map<'a, &'a str, Module>,
    #[cfg(feature = "modules")]
    pub module_idx: Item<'a, u64>,
    #[cfg(feature = "withdraw")]
    pub withdrawable_tokens: Map<'a, &'a str, AssetInfo>,
}

impl<'a> Default for ADOContract<'a> {
    fn default() -> Self {
        ADOContract {
            owner: Item::new("owner"),
            operators: Map::new("operators"),
            ado_type: Item::new("ado_type"),
            mission_contract: Item::new("mission_contract"),
            #[cfg(feature = "primitive")]
            primitive_contract: Item::new("primitive_contract"),
            #[cfg(feature = "primitive")]
            cached_addresses: Map::new("cached_addresses"),
            #[cfg(feature = "modules")]
            module_info: Map::new("andr_modules"),
            #[cfg(feature = "modules")]
            module_idx: Item::new("andr_module_idx"),
            #[cfg(feature = "withdraw")]
            withdrawable_tokens: Map::new("withdrawable_tokens"),
        }
    }
}

impl<'a> ADOContract<'a> {
    /// Helper function to query if a given address is a operator.
    ///
    /// Returns a boolean value indicating if the given address is a operator.
    pub fn is_operator(&self, storage: &dyn Storage, addr: &str) -> bool {
        self.operators.has(storage, addr)
    }

    /// Helper function to query if a given address is the current contract owner.
    ///
    /// Returns a boolean value indicating if the given address is the contract owner.
    pub fn is_contract_owner(
        &self,
        storage: &dyn Storage,
        addr: &str,
    ) -> Result<bool, ContractError> {
        let owner = self.owner.load(storage)?;
        Ok(addr == owner)
    }

    pub fn is_owner_or_operator(
        &self,
        storage: &dyn Storage,
        addr: &str,
    ) -> Result<bool, ContractError> {
        Ok(self.is_contract_owner(storage, addr)? || self.is_operator(storage, addr))
    }

    pub fn get_mission_contract(
        &self,
        storage: &dyn Storage,
    ) -> Result<Option<Addr>, ContractError> {
        Ok(self.mission_contract.may_load(storage)?)
    }

    pub(crate) fn initialize_operators(
        &self,
        storage: &mut dyn Storage,
        operators: Vec<String>,
    ) -> Result<(), ContractError> {
        for operator in operators.iter() {
            self.operators.save(storage, operator, &true)?;
        }
        Ok(())
    }

    pub(crate) fn is_nested(&self, data: &Option<Binary>) -> bool {
        let res: Result<QueryMsg, ContractError> = parse_message(data);
        res.is_ok()
    }
}
