#[cfg(feature = "rates")]
use {
    crate::ado_base::rates::{LocalRate, Rate},
    crate::amp::Recipient,
    cw_storage_plus::Path,
    std::ops::Deref,
};

use crate::{
    ado_base::{
        permissioning::{LocalPermission, Permission},
        AndromedaMsg, InstantiateMsg,
    },
    ado_contract::ADOContract,
    amp::{addresses::AndrAddr, messages::AMPPkt},
    common::{context::ExecuteContext, reply::ReplyId},
    error::{from_semver, ContractError},
    os::{aos_querier::AOSQuerier, economics::ExecuteMsg as EconomicsExecuteMsg},
};
use cosmwasm_std::{
    attr, ensure, from_json, to_json_binary, Addr, Api, ContractInfoResponse, CosmosMsg, Deps,
    DepsMut, Env, MessageInfo, QuerierWrapper, Response, StdError, Storage, SubMsg, WasmMsg,
};
use cw2::{get_contract_version, set_contract_version};
use semver::Version;
use serde::{de::DeserializeOwned, Serialize};

type ExecuteContextFunction<M, E> = fn(ExecuteContext, M) -> Result<Response, E>;

impl ADOContract<'_> {
    pub fn instantiate(
        &self,
        storage: &mut dyn Storage,
        env: Env,
        api: &dyn Api,
        querier: &QuerierWrapper,
        info: MessageInfo,
        msg: InstantiateMsg,
    ) -> Result<Response, ContractError> {
        let ado_type = if msg.ado_type.starts_with("crates.io:andromeda-") {
            msg.ado_type.strip_prefix("crates.io:andromeda-").unwrap()
        } else if msg.ado_type.starts_with("crates.io:") {
            msg.ado_type.strip_prefix("crates.io:").unwrap()
        } else {
            &msg.ado_type
        };
        cw2::set_contract_version(storage, ado_type, msg.ado_version)?;
        let mut owner = api.addr_validate(&msg.owner.unwrap_or(info.sender.to_string()))?;
        self.original_publisher.save(storage, &info.sender)?;
        self.block_height.save(storage, &env.block.height)?;
        self.ado_type.save(storage, &ado_type.to_string())?;
        self.kernel_address
            .save(storage, &api.addr_validate(&msg.kernel_address)?)?;
        let mut attributes = vec![
            attr("method", "instantiate"),
            attr("type", ado_type),
            attr("kernel_address", msg.kernel_address),
        ];

        // We do not want to store app contracts for the kernel, exit early if current contract is kernel
        let is_kernel_contract = ado_type.contains("kernel");
        if is_kernel_contract {
            self.owner.save(storage, &owner)?;
            attributes.push(attr("owner", owner));
            return Ok(Response::new().add_attributes(attributes));
        }

        // Check if the sender is an app contract to allow for automatic storage of app contrcat reference
        let maybe_contract_info = querier.query_wasm_contract_info(info.sender.clone());
        let is_sender_contract = maybe_contract_info.is_ok();
        if is_sender_contract {
            let ContractInfoResponse { code_id, .. } = maybe_contract_info?;
            let sender_ado_type = AOSQuerier::ado_type_getter(
                querier,
                &self.get_adodb_address(storage, querier)?,
                code_id,
            )?;
            let is_sender_app = Some("app-contract".to_string()) == sender_ado_type;
            // Automatically save app contract reference if creator is an app contract
            if is_sender_app {
                self.app_contract
                    .save(storage, &Addr::unchecked(info.sender.to_string()))?;
                let app_owner = AOSQuerier::ado_owner_getter(querier, &info.sender)?;
                owner = app_owner;
                attributes.push(attr("app_contract", info.sender.to_string()));
            }
        }

        self.owner.save(storage, &owner)?;
        attributes.push(attr("owner", owner));
        Ok(Response::new().add_attributes(attributes))
    }

    /// Handles execution of ADO specific messages.
    pub fn execute(
        &self,
        ctx: ExecuteContext,
        msg: impl Serialize,
    ) -> Result<Response, ContractError> {
        let msg = to_json_binary(&msg)?;
        match from_json::<AndromedaMsg>(&msg) {
            Ok(msg) => match msg {
                AndromedaMsg::Ownership(msg) => {
                    self.execute_ownership(ctx.deps, ctx.env, ctx.info, msg)
                }
                AndromedaMsg::UpdateAppContract { address } => {
                    self.execute_update_app_contract(ctx.deps, ctx.info, address, None)
                }
                #[cfg(feature = "rates")]
                AndromedaMsg::Rates(rates_message) => self.execute_rates(ctx, rates_message),
                AndromedaMsg::UpdateKernelAddress { address } => {
                    self.update_kernel_address(ctx.deps, ctx.info, address)
                }
                AndromedaMsg::Permissioning(msg) => self.execute_permissioning(ctx, msg),
                AndromedaMsg::AMPReceive(_) => panic!("AMP Receive should be handled separately"),
            },
            _ => Err(ContractError::NotImplemented { msg: None }),
        }
    }

    pub fn migrate(
        &self,
        mut deps: DepsMut,
        env: Env,
        contract_name: &str,
        contract_version: &str,
    ) -> Result<Response, ContractError> {
        // New version
        let version: Version = contract_version.parse().map_err(from_semver)?;

        // Old version
        let stored = get_contract_version(deps.storage)?;
        let storage_version: Version = stored.version.parse().map_err(from_semver)?;
        let contract_name = if contract_name.starts_with("crates.io:andromeda-") {
            contract_name.strip_prefix("crates.io:andromeda-").unwrap()
        } else if contract_name.starts_with("crates.io:") {
            contract_name.strip_prefix("crates.io:").unwrap()
        } else {
            contract_name
        };
        ensure!(
            stored.contract == contract_name,
            ContractError::CannotMigrate {
                previous_contract: stored.contract,
            }
        );

        // New version has to be newer/greater than the old version
        ensure!(
            storage_version <= version,
            ContractError::CannotMigrate {
                previous_contract: stored.version,
            }
        );
        let owner = self.owner.load(deps.storage)?;

        // Get all permissioned actions and actors in one pass
        let permissioned_actions = self.query_permissioned_actions(deps.as_ref())?;
        if !permissioned_actions.is_empty() {
            // Iterate through all actions
            for action in &permissioned_actions {
                let actors = self.query_permissioned_actors(
                    deps.as_ref(),
                    action.clone(),
                    None,
                    None,
                    None,
                )?;

                for actor in actors {
                    // Check permission structure for each actor
                    let permissions = self.query_permissions(deps.as_ref(), &actor, None, None)?;
                    // Iterate through all permissions instead of just checking the first one
                    for permission in permissions {
                        let local_permission = permission
                            .permission
                            .clone()
                            .get_permission(deps.as_ref(), actor.as_str())?;

                        // Check if using old permission structure (without 'start' variant)
                        let json_str = String::from_utf8(to_json_binary(&local_permission)?.0)?;
                        if !json_str.contains("start") {
                            let ctx = ExecuteContext::new(
                                deps.branch(),
                                MessageInfo {
                                    sender: owner.clone(),
                                    funds: vec![],
                                },
                                env.clone(),
                            );

                            // Determine permission type from JSON structure
                            if json_str.contains("whitelisted") {
                                self.execute_set_permission(
                                    ctx,
                                    vec![AndrAddr::from_string(actor.clone())],
                                    action.clone(),
                                    Permission::Local(LocalPermission::Whitelisted {
                                        start: None,
                                        expiration: None,
                                    }),
                                )?;
                            } else if json_str.contains("blacklisted") {
                                self.execute_set_permission(
                                    ctx,
                                    vec![AndrAddr::from_string(actor.clone())],
                                    action.clone(),
                                    Permission::Local(LocalPermission::Blacklisted {
                                        start: None,
                                        expiration: None,
                                    }),
                                )?;
                            } else if json_str.contains("limited") {
                                self.execute_set_permission(
                                    ctx,
                                    vec![AndrAddr::from_string(actor.clone())],
                                    action.clone(),
                                    Permission::Local(LocalPermission::Limited {
                                        start: None,
                                        expiration: None,
                                        uses: 0,
                                    }),
                                )?;
                            }
                        } else {
                            // If one permission is up to date, we assume that all of them are
                            break;
                        }
                    }
                }
            }
        }

        #[cfg(feature = "rates")]
        {
            let all_rates = self.get_all_rates(deps.as_ref())?;
            for (action, rate) in all_rates.all_rates {
                match rate {
                    Rate::Local(local_rate) => {
                        // Remove if recipient is in old Vec<Recipient> format
                        if from_json::<Vec<Recipient>>(&to_json_binary(&local_rate.recipient)?)
                            .is_ok()
                        {
                            // Clearing all rates assuming that if one needs to be removed then all of them should be removed
                            self.rates.clear(deps.storage);
                        }
                        // One iteration is enough since the rates are either all valid or invalid
                        break;
                    }
                    Rate::Contract(andr_addr) => {
                        let contract_addr = andr_addr.get_raw_address(&deps.as_ref())?;
                        let key_path: Path<Vec<u8>> =
                            Path::new("rates".as_bytes(), &[action.as_bytes()]);

                        if let Some(remote_rate) = deps
                            .querier
                            .query_wasm_raw(&contract_addr, key_path.deref())?
                        {
                            // Remove if remote rate's recipient is in old Vec<Recipient> format
                            if let Ok(local_rate) = from_json::<LocalRate>(&remote_rate) {
                                if from_json::<Vec<Recipient>>(&to_json_binary(
                                    &local_rate.recipient,
                                )?)
                                .is_ok()
                                {
                                    self.rates.clear(deps.branch().storage);
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }

        set_contract_version(deps.branch().storage, contract_name, contract_version)?;
        Ok(Response::default())
    }

    /// Validates all provided `AndrAddr` addresses.
    ///
    /// Requires the VFS address to be set if any address is a VFS path.
    /// Automatically validates all stored modules.
    pub fn validate_andr_addresses(
        &self,
        deps: &Deps,
        addresses: Vec<AndrAddr>,
    ) -> Result<(), ContractError> {
        let vfs_address = self.get_vfs_address(deps.storage, &deps.querier);
        match vfs_address {
            Ok(vfs_address) => {
                for address in addresses {
                    self.validate_andr_address(deps, address, vfs_address.clone())?;
                }
                Ok(())
            }
            Err(_) => {
                for address in addresses {
                    ensure!(address.is_addr(deps.api), ContractError::InvalidAddress {});
                }
                Ok(())
            }
        }
    }

    /// Validates the given `AndrAddr` address.
    pub(crate) fn validate_andr_address(
        &self,
        deps: &Deps,
        address: AndrAddr,
        vfs_address: Addr,
    ) -> Result<(), ContractError> {
        address.validate(deps.api)?;
        if !address.is_addr(deps.api) {
            address.get_raw_address_from_vfs(deps, vfs_address)?;
        }
        Ok(())
    }

    #[inline]
    /// Gets the stored address for the Kernel contract
    pub fn get_kernel_address(&self, storage: &dyn Storage) -> Result<Addr, ContractError> {
        let kernel_address = self.kernel_address.load(storage)?;
        Ok(kernel_address)
    }

    #[inline]
    /// Gets the current address for the VFS contract.
    pub fn get_vfs_address(
        &self,
        storage: &dyn Storage,
        querier: &QuerierWrapper,
    ) -> Result<Addr, ContractError> {
        let kernel_address = self.get_kernel_address(storage)?;
        AOSQuerier::vfs_address_getter(querier, &kernel_address)
    }

    #[inline]
    /// Gets the current address for the VFS contract.
    pub fn get_adodb_address(
        &self,
        storage: &dyn Storage,
        querier: &QuerierWrapper,
    ) -> Result<Addr, ContractError> {
        let kernel_address = self.get_kernel_address(storage)?;
        AOSQuerier::adodb_address_getter(querier, &kernel_address)
    }

    /// Handles receiving and verifies an AMPPkt from the Kernel before executing the appropriate messages.
    ///
    /// Calls the provided handler with the AMP packet attached within the context.
    pub fn execute_amp_receive<M: DeserializeOwned, E>(
        &self,
        ctx: ExecuteContext,
        mut packet: AMPPkt,
        handler: ExecuteContextFunction<M, E>,
    ) -> Result<Response, E>
    where
        E: From<ContractError> + From<StdError>,
    {
        packet.verify_origin(&ctx.info, &ctx.deps.as_ref())?;
        let ctx = ctx.with_ctx(packet.clone());
        let msg_opt = packet.messages.pop();
        if let Some(msg_opt) = msg_opt {
            let msg: M = from_json(msg_opt.clone().message)?;
            let response = handler(ctx, msg)?;

            Ok(response)
        } else {
            Err(ContractError::InvalidPacket {
                error: Some("AMP Packet received with no messages".to_string()),
            }
            .into())
        }
    }

    /// Generates a message to pay a fee for a given action by the given payee
    ///
    /// Fees are paid in the following fallthrough priority:
    /// 1. ADO Contract
    /// 2. App Contract for sending ADO
    /// 3. Provided Payee
    ///
    /// If any of the above cannot pay the fee the remainder is paid by the next in the list until no remainder remains.
    /// If there is still a remainder after all 3 payments then the fee cannot be paid and the message will error.
    pub fn pay_fee(
        &self,
        storage: &dyn Storage,
        querier: &QuerierWrapper,
        action: String,
        payee: Addr,
    ) -> Result<SubMsg, ContractError> {
        let kernel_address = self.get_kernel_address(storage)?;
        let economics_contract_address =
            AOSQuerier::kernel_address_getter(querier, &kernel_address, "economics")?;
        let economics_msg = EconomicsExecuteMsg::PayFee { action, payee };
        let msg = SubMsg::reply_on_error(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: economics_contract_address.to_string(),
                msg: to_json_binary(&economics_msg)?,
                funds: vec![],
            }),
            ReplyId::PayFee.repr(),
        );

        Ok(msg)
    }

    /// Updates the current kernel address used by the ADO
    /// Requires the sender to be the owner of the ADO
    pub fn update_kernel_address(
        &self,
        deps: DepsMut,
        info: MessageInfo,
        address: Addr,
    ) -> Result<Response, ContractError> {
        ensure!(
            self.is_contract_owner(deps.storage, info.sender.as_str())?,
            ContractError::Unauthorized {}
        );
        self.kernel_address.save(deps.storage, &address)?;
        Ok(Response::new()
            .add_attribute("action", "update_kernel_address")
            .add_attribute("address", address))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mock_querier::MOCK_KERNEL_CONTRACT;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};

    mod app_contract {
        use super::*;

        #[test]
        fn test_update_app_contract() {
            let contract = ADOContract::default();
            let mut deps = mock_dependencies();

            let info = mock_info("owner", &[]);
            let deps_mut = deps.as_mut();
            contract
                .instantiate(
                    deps_mut.storage,
                    mock_env(),
                    deps_mut.api,
                    &deps_mut.querier,
                    info.clone(),
                    InstantiateMsg {
                        ado_type: "type".to_string(),
                        ado_version: "version".to_string(),
                        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                        owner: None,
                    },
                )
                .unwrap();

            let address = String::from("address");
            let msg = AndromedaMsg::UpdateAppContract {
                address: address.clone(),
            };

            let res = contract
                .execute(ExecuteContext::new(deps.as_mut(), info, mock_env()), msg)
                .unwrap();

            assert_eq!(
                Response::new()
                    .add_attribute("action", "update_app_contract")
                    .add_attribute("address", address),
                res
            );
        }
    }

    mod permissions {
        use super::*;
        use cosmwasm_schema::cw_serde;
        use cosmwasm_std::to_json_binary;
        use rstest::rstest;

        // Old permission structure without 'start'
        #[cw_serde]
        enum OldLocalPermission {
            Whitelisted { expiration: Option<u64> },
            Blacklisted { expiration: Option<u64> },
            Limited { expiration: Option<u64>, uses: u64 },
        }

        #[rstest]
        #[case(
            OldLocalPermission::Whitelisted { expiration: None },
            LocalPermission::Whitelisted { start: None, expiration: None },
            "whitelisted"
        )]
        #[case(
            OldLocalPermission::Blacklisted { expiration: None },
            LocalPermission::Blacklisted { start: None, expiration: None },
            "blacklisted"
        )]
        #[case(
            OldLocalPermission::Limited { expiration: None, uses: 5 },
            LocalPermission::Limited { start: None, expiration: None, uses: 5 },
            "limited"
        )]
        fn test_permission_formats(
            #[case] old_permission: OldLocalPermission,
            #[case] new_permission: LocalPermission,
            #[case] permission_type: &str,
        ) {
            // Serialize old and new permissions to JSON
            let old_json = String::from_utf8(to_json_binary(&old_permission).unwrap().0).unwrap();
            let new_json = String::from_utf8(to_json_binary(&new_permission).unwrap().0).unwrap();

            // Assert old format JSON
            assert!(!old_json.contains("start"));
            assert!(old_json.contains(permission_type));

            // Assert new format JSON
            assert!(new_json.contains("start"));
            assert!(new_json.contains(permission_type));
        }
    }
    #[cfg(feature = "rates")]
    mod rates {
        use super::*;

        use crate::ado_base::rates::{LocalRate, LocalRateType, PercentRate};
        use cosmwasm_std::Decimal;

        #[test]
        fn test_rates_migration() {
            let contract = ADOContract::default();
            let mut deps = mock_dependencies();

            // Setup initial contract state
            let info = mock_info("owner", &[]);
            let deps_mut = deps.as_mut();
            contract
                .instantiate(
                    deps_mut.storage,
                    mock_env(),
                    deps_mut.api,
                    &deps_mut.querier,
                    info.clone(),
                    InstantiateMsg {
                        ado_type: "marketplace".to_string(),
                        ado_version: "1.0.0".to_string(),
                        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                        owner: None,
                    },
                )
                .unwrap();

            // Set up a test rate
            let rate = LocalRate {
                rate_type: LocalRateType::Additive,
                recipient: Recipient::from_string("recipient"),
                value: crate::ado_base::rates::LocalRateValue::Percent(PercentRate {
                    percent: Decimal::one(),
                }),
                description: None,
            };

            // Save the rate in storage
            contract
                .rates
                .save(deps.as_mut().storage, "Claim", &Rate::Local(rate))
                .unwrap();

            // Verify rate is saved
            let saved_rates = contract.get_all_rates(deps.as_ref()).unwrap();
            assert_eq!(saved_rates.all_rates.len(), 1);

            // Perform migration
            contract
                .migrate(deps.as_mut(), mock_env(), "marketplace", "2.0.0")
                .unwrap();

            // Verify rates were handled correctly during migration
            let post_migration_rates = contract.get_all_rates(deps.as_ref()).unwrap();
            assert_eq!(post_migration_rates.all_rates.len(), 1);
        }
    }

    mod permissions_migration {
        use super::*;
        use crate::ado_base::permissioning::LocalPermission;

        #[test]
        fn test_permissions_migration() {
            let contract = ADOContract::default();
            let mut deps = mock_dependencies();

            // Setup initial contract state
            let info = mock_info("owner", &[]);
            let deps_mut = deps.as_mut();
            contract
                .instantiate(
                    deps_mut.storage,
                    mock_env(),
                    deps_mut.api,
                    &deps_mut.querier,
                    info.clone(),
                    InstantiateMsg {
                        ado_type: "marketplace".to_string(),
                        ado_version: "1.0.0".to_string(),
                        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                        owner: None,
                    },
                )
                .unwrap();

            // Set up a test permission
            let permission = Permission::Local(LocalPermission::Whitelisted {
                start: None,
                expiration: None,
            });

            // Save the permission in storage
            let ctx = ExecuteContext::new(deps.as_mut(), info, mock_env());
            contract
                .execute_set_permission(
                    ctx,
                    vec![AndrAddr::from_string("actor")],
                    "test_action".to_string(),
                    permission,
                )
                .unwrap();

            // Verify permission is saved
            let saved_permissions = contract
                .query_permissions(deps.as_ref(), "actor", None, None)
                .unwrap();
            assert_eq!(saved_permissions.len(), 1);

            // Perform migration
            contract
                .migrate(deps.as_mut(), mock_env(), "marketplace", "2.0.0")
                .unwrap();

            // Verify permissions were handled correctly during migration
            let post_migration_permissions = contract
                .query_permissions(deps.as_ref(), "actor", None, None)
                .unwrap();
            assert_eq!(post_migration_permissions.len(), 1);
        }
    }
}
