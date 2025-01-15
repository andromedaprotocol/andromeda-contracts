use crate::ado_base::permissioning::LocalPermission;
use crate::amp::{ADO_DB_KEY, IBC_REGISTRY_KEY, VFS_KEY};
use crate::error::ContractError;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{from_json, Addr, ChannelResponse, IbcQuery, QuerierWrapper};
use cw_storage_plus::Path;
use lazy_static::__Deref;
use serde::de::DeserializeOwned;
use std::str::from_utf8;

#[cfg(feature = "rates")]
use crate::ado_base::rates::LocalRate;

use super::adodb::{ADOVersion, ActionFee, QueryMsg as ADODBQueryMsg};
use super::ibc_registry::{
    hops_to_path, path_to_hops, DenomInfo, DenomInfoResponse, Hop, QueryMsg as IBCRegistryQueryMsg,
};
use super::kernel::ChannelInfo;
use super::TRANSFER_PORT;

#[cw_serde]
pub struct AOSQuerier();

impl AOSQuerier {
    // namespace -> storage key
    // key_name -> item key
    // Taken from: https://github.com/KompleTeam/komple-framework/blob/387d333af03e794927b8ef8ac536d2a42ae7a1ff/packages/utils/src/storage.rs#L25
    pub fn get_map_storage_key(
        namespace: &str,
        key_bytes: &[&[u8]],
    ) -> Result<String, ContractError> {
        let namespace_bytes = namespace.as_bytes();
        let path: Path<Vec<u32>> = Path::new(namespace_bytes, key_bytes);
        let path_str = from_utf8(path.deref())?;
        Ok(path_str.to_string())
    }

    // To find the key value in storage, we need to construct a path to the key
    // For Map storage this key is generated with get_map_storage_key
    // For Item storage this key is the namespace value
    pub fn query_storage<T>(
        querier: &QuerierWrapper,
        addr: &Addr,
        key: &str,
    ) -> Result<Option<T>, ContractError>
    where
        T: DeserializeOwned,
    {
        let data = querier.query_wasm_raw(addr, key.as_bytes())?;
        match data {
            Some(data) => {
                let res = from_utf8(&data)?;
                let res = from_json(res.as_bytes())?;
                Ok(Some(res))
            }
            None => Ok(None),
        }
    }

    pub fn ado_type_getter(
        querier: &QuerierWrapper,
        adodb_addr: &Addr,
        code_id: u64,
    ) -> Result<Option<String>, ContractError> {
        let key = AOSQuerier::get_map_storage_key("ado_type", &[code_id.to_string().as_bytes()])?;
        let ado_type: Option<ADOVersion> = AOSQuerier::query_storage(querier, adodb_addr, &key)?;
        Ok(ado_type.map(|v| v.get_type()))
    }

    pub fn ado_type_getter_smart(
        querier: &QuerierWrapper,
        adodb_addr: &Addr,
        code_id: u64,
    ) -> Result<Option<String>, ContractError> {
        let query = ADODBQueryMsg::ADOType { code_id };
        let ado_type: Option<ADOVersion> = querier.query_wasm_smart(adodb_addr, &query)?;
        Ok(ado_type.map(|v| v.get_type()))
    }

    pub fn ado_publisher_getter(
        querier: &QuerierWrapper,
        adodb_addr: &Addr,
        ado_type: &str,
    ) -> Result<String, ContractError> {
        let key = AOSQuerier::get_map_storage_key("publisher", &[ado_type.as_bytes()])?;
        let verify: Option<String> = AOSQuerier::query_storage(querier, adodb_addr, &key)?;

        match verify {
            Some(publisher) => Ok(publisher),
            None => Err(ContractError::InvalidAddress {}),
        }
    }

    /// Checks if the code id exists in the ADODB by querying its raw storage for the code id's ado type
    pub fn verify_code_id(
        querier: &QuerierWrapper,
        adodb_addr: &Addr,
        code_id: u64,
    ) -> Result<(), ContractError> {
        let key = AOSQuerier::get_map_storage_key("ado_type", &[code_id.to_string().as_bytes()])?;
        let verify: Option<String> = AOSQuerier::query_storage(querier, adodb_addr, &key)?;

        if verify.is_some() {
            Ok(())
        } else {
            Err(ContractError::Unauthorized {})
        }
    }

    pub fn code_id_getter_raw(
        querier: &QuerierWrapper,
        adodb_addr: &Addr,
        ado_type: &str,
    ) -> Result<u64, ContractError> {
        let key = AOSQuerier::get_map_storage_key("code_id", &[ado_type.as_bytes()])?;
        let verify: Option<u64> = AOSQuerier::query_storage(querier, adodb_addr, &key)?;

        match verify {
            Some(code_id) => Ok(code_id),
            None => Err(ContractError::InvalidAddress {}),
        }
    }

    pub fn code_id_getter(
        querier: &QuerierWrapper,
        adodb_addr: &Addr,
        ado_type: &str,
    ) -> Result<u64, ContractError> {
        let query = ADODBQueryMsg::CodeId {
            key: ado_type.to_string(),
        };
        let code_id: u64 = querier.query_wasm_smart(adodb_addr, &query)?;
        Ok(code_id)
    }

    /// Queries the kernel's raw storage for the VFS's address
    pub fn vfs_address_getter(
        querier: &QuerierWrapper,
        kernel_addr: &Addr,
    ) -> Result<Addr, ContractError> {
        AOSQuerier::kernel_address_getter(querier, kernel_addr, VFS_KEY)
    }

    /// Queries the kernel's raw storage for the ADODB's address
    pub fn adodb_address_getter(
        querier: &QuerierWrapper,
        kernel_addr: &Addr,
    ) -> Result<Addr, ContractError> {
        AOSQuerier::kernel_address_getter(querier, kernel_addr, ADO_DB_KEY)
    }

    /// Queries the kernel's raw storage for the IBC Registry's address
    pub fn ibc_registry_address_getter(
        querier: &QuerierWrapper,
        kernel_addr: &Addr,
    ) -> Result<Addr, ContractError> {
        AOSQuerier::kernel_address_getter(querier, kernel_addr, IBC_REGISTRY_KEY)
    }

    /// Queries the kernel's raw storage for the VFS's address
    pub fn kernel_address_getter(
        querier: &QuerierWrapper,
        kernel_addr: &Addr,
        key: &str,
    ) -> Result<Addr, ContractError> {
        let key = AOSQuerier::get_map_storage_key("kernel_addresses", &[key.as_bytes()])?;
        let verify: Option<Addr> = AOSQuerier::query_storage(querier, kernel_addr, &key)?;
        match verify {
            Some(address) => Ok(address),
            None => Err(ContractError::InvalidAddress {}),
        }
    }

    pub fn action_fee_getter(
        querier: &QuerierWrapper,
        adodb_addr: &Addr,
        ado_type: &str,
        action: &str,
    ) -> Result<Option<ActionFee>, ContractError> {
        let key = AOSQuerier::get_map_storage_key(
            "action_fees",
            &[ado_type.as_bytes(), action.as_bytes()],
        )?;
        let fee: Option<ActionFee> = AOSQuerier::query_storage(querier, adodb_addr, &key)?;
        Ok(fee)
    }

    /// Queries the kernel's raw storage for the VFS's address
    pub fn ado_owner_getter(
        querier: &QuerierWrapper,
        ado_addr: &Addr,
    ) -> Result<Addr, ContractError> {
        let verify: Option<Addr> = AOSQuerier::query_storage(querier, ado_addr, "owner")?;
        match verify {
            Some(address) => Ok(address),
            None => Err(ContractError::InvalidAddress {}),
        }
    }

    /// Queries the current chain name from the kernel
    pub fn get_current_chain(
        querier: &QuerierWrapper,
        kernel_addr: &Addr,
    ) -> Result<String, ContractError> {
        let verify: Option<String> =
            AOSQuerier::query_storage(querier, kernel_addr, "kernel_curr_chain")?;
        match verify {
            Some(chain) => Ok(chain),
            None => Err(ContractError::InvalidAddress {}),
        }
    }

    /// Queries the current chain name from the kernel
    pub fn get_chain_info(
        querier: &QuerierWrapper,
        kernel_addr: &Addr,
        chain_name: &str,
    ) -> Result<ChannelInfo, ContractError> {
        let key = AOSQuerier::get_map_storage_key("kernel_channels", &[chain_name.as_bytes()])?;
        let verify: Option<ChannelInfo> =
            AOSQuerier::query_storage(querier, kernel_addr, key.as_str())?;
        match verify {
            Some(chain) => Ok(chain),
            None => Err(ContractError::InvalidAddress {}),
        }
    }
    /// Queries an actor's permission from the address list contract
    pub fn get_permission(
        querier: &QuerierWrapper,
        contract_addr: &Addr,
        actor: &str,
    ) -> Result<LocalPermission, ContractError> {
        let key = AOSQuerier::get_map_storage_key("permissioning", &[actor.as_bytes()])?;
        let permission: Option<LocalPermission> =
            AOSQuerier::query_storage(querier, contract_addr, key.as_str())?;
        match permission {
            Some(permission) => Ok(permission),
            None => Err(ContractError::InvalidAddress {}),
        }
    }

    #[cfg(feature = "rates")]
    /// Queries the rates contract
    pub fn get_rate(
        querier: &QuerierWrapper,
        addr: &Addr,
        action: &str,
    ) -> Result<LocalRate, ContractError> {
        let key = AOSQuerier::get_map_storage_key("rates", &[action.as_bytes()])?;
        let verify: Option<LocalRate> = AOSQuerier::query_storage(querier, addr, key.as_str())?;
        match verify {
            Some(rate) => Ok(rate),
            None => Err(ContractError::InvalidAddress {}),
        }
    }

    pub fn denom_trace_getter(
        querier: &QuerierWrapper,
        ibc_registry_addr: &Addr,
        denom: &str,
    ) -> Result<DenomInfo, ContractError> {
        let query = IBCRegistryQueryMsg::DenomInfo {
            denom: denom.to_string(),
        };
        let denom_info_response: DenomInfoResponse =
            querier.query_wasm_smart(ibc_registry_addr, &query)?;
        Ok(denom_info_response.denom_info)
    }

    // #[cfg(feature = "ibc")]
    pub fn get_counterparty_denom(
        querier: &QuerierWrapper,
        denom_trace: &DenomInfo,
        src_channel: &str,
    ) -> Result<(String, DenomInfo), ContractError> {
        let mut hops = path_to_hops(denom_trace.path.clone())?;
        let last_hop = hops.last();
        if let Some(hop) = last_hop {
            // If the last hop was done via the port we are transferring,then we need to unwrap it
            if hop.port_id == TRANSFER_PORT && hop.channel_id == src_channel {
                // Remove the last hop
                hops.pop();

                let new_denom_trace = DenomInfo {
                    path: hops_to_path(hops),
                    base_denom: denom_trace.base_denom.clone(),
                };
                let new_denom = if new_denom_trace.path.is_empty() {
                    new_denom_trace.base_denom.clone()
                } else {
                    new_denom_trace.get_ibc_denom()
                };
                return Ok((new_denom, new_denom_trace));
            }
        };

        // Otherwise, we need to get the counterparty port id and add it as last hop
        let channel_info_msg = IbcQuery::Channel {
            channel_id: src_channel.to_string(),
            port_id: Some(TRANSFER_PORT.to_string()),
        };
        let channel_info: ChannelResponse =
            querier.query(&cosmwasm_std::QueryRequest::Ibc(channel_info_msg))?;

        let counterparty = channel_info
            .channel
            .ok_or(ContractError::InvalidDenomTracePath {
                path: denom_trace.path.clone(),
                msg: Some("Channel info not found".to_string()),
            })?
            .counterparty_endpoint;

        hops.push(Hop {
            port_id: counterparty.port_id,
            channel_id: counterparty.channel_id,
        });

        let new_denom_trace = DenomInfo {
            path: hops_to_path(hops),
            base_denom: denom_trace.base_denom.clone(),
        };
        Ok((new_denom_trace.get_ibc_denom(), new_denom_trace))
    }
}
