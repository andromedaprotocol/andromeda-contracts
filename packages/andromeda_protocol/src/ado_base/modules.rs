use crate::{
    communication::{encode_binary, query_get},
    error::ContractError,
    factory::CodeIdResponse,
    primitive::{get_address, AndromedaContract},
    require,
};
use cosmwasm_std::{Binary, CosmosMsg, QuerierWrapper, ReplyOn, Storage, SubMsg, WasmMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An enum describing the different available modules for any Andromeda Token contract
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModuleType {
    Rates,
    Offers,
    AddressList,
    Auction,
    Receipt,
    /// Used for external contracts, undocumented
    Other,
}

/// Conversion from a module type to string, primarily used to query code ids from our factory contract
impl From<ModuleType> for String {
    fn from(module_type: ModuleType) -> Self {
        match module_type {
            ModuleType::Receipt => String::from("receipt"),
            ModuleType::AddressList => String::from("address_list"),
            ModuleType::Rates => String::from("rates"),
            ModuleType::Auction => String::from("auction"),
            ModuleType::Offers => String::from("offers"),
            ModuleType::Other => String::from("other"),
        }
    }
}

/// Modules can be instantiated in two different ways
/// New - Provide an instantiation message for the contract, a new contract will be instantiated and the address recorded
/// Address - Provide an address for an already instantiated module contract
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstantiateType {
    New(Binary),
    Address(String),
}

/// A struct describing a token module, provided with the instantiation message this struct is used to record the info about the module and how/if it should be instantiated
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Module {
    pub module_type: ModuleType,
    pub instantiate: InstantiateType,
    pub is_mutable: bool,
}

/// Struct used to represent a module and its currently recorded address
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ModuleInfoWithAddress {
    pub module: Module,
    pub address: String,
}

/// The type of ADO that is using these modules.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum ADOType {
    CW721,
    CW20,
}

impl Module {
    /// Queries the code id for a module from the factory contract
    pub fn get_code_id(
        &self,
        storage: &dyn Storage,
        querier: QuerierWrapper,
    ) -> Result<Option<u64>, ContractError> {
        let factory_address = get_address(storage, querier, AndromedaContract::Factory)?;
        match self.module_type {
            ModuleType::Other => Ok(None),
            _ => {
                let code_id_resp: CodeIdResponse = query_get(
                    Some(encode_binary(&String::from(self.module_type.clone()))?),
                    factory_address,
                    &querier,
                )?;
                Ok(Some(code_id_resp.code_id))
            }
        }
    }

    /// Generate an instantiation message for the module if its required
    pub fn generate_instantiate_msg(
        &self,
        storage: &dyn Storage,
        querier: QuerierWrapper,
        module_id: u64,
    ) -> Result<Option<SubMsg>, ContractError> {
        if let InstantiateType::New(msg) = &self.instantiate {
            match self.get_code_id(storage, querier)? {
                None => Err(ContractError::InvalidModule {
                    msg: Some(String::from(
                        "Module type provided does not have a valid Code Id",
                    )),
                }),
                Some(code_id) => Ok(Some(SubMsg {
                    id: module_id,
                    reply_on: ReplyOn::Always,
                    msg: CosmosMsg::Wasm(WasmMsg::Instantiate {
                        admin: None,
                        code_id,
                        msg: msg.clone(),
                        funds: vec![],
                        label: format!("Instantiate: {}", String::from(self.module_type.clone())),
                    }),
                    gas_limit: None,
                })),
            }
        } else {
            Ok(None)
        }
    }

    /// Validates `self` by checking that it is unique, does not conflict with any other module,
    /// and does not conflict with the creating ADO.
    pub fn validate(&self, modules: &[Module], ado_type: &ADOType) -> Result<(), ContractError> {
        require(self.is_unique(modules), ContractError::ModuleNotUnique {})?;

        if ado_type == &ADOType::CW20 && contains_module(modules, ModuleType::Auction) {
            return Err(ContractError::IncompatibleModules {
                msg: "An Auction module cannot be used for a CW20 ADO".to_string(),
            });
        }

        Ok(())
    }

    /// Determines if `self` is unique within the context of a vector of `Module`
    ///
    /// ## Arguments
    /// * `all_modules` - The vector of modules containing the provided module
    ///
    /// Returns a `boolean` representing whether the module is unique or not
    fn is_unique(&self, all_modules: &[Module]) -> bool {
        let mut total = 0;
        all_modules.iter().for_each(|m| {
            if self.module_type == m.module_type {
                total += 1;
            }
        });

        total == 1
    }
}

/// Checks if any element of `modules` contains one of type `module_type`.
fn contains_module(modules: &[Module], module_type: ModuleType) -> bool {
    modules.iter().any(|m| m.module_type == module_type)
}
