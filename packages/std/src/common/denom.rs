use std::fmt::{Display, Formatter, Result as StdResult};

use crate::{
    ado_base::permissioning::{LocalPermission, Permission},
    ado_contract::ADOContract,
    amp::AndrAddr,
    error::ContractError,
};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    attr, coin, ensure, to_json_binary, wasm_execute, BankMsg, Deps, DepsMut, Env, MessageInfo,
    QueryRequest, Response, SubMsg, Uint128, WasmQuery,
};
use cw20::{Cw20ExecuteMsg, Cw20QueryMsg, TokenInfoResponse};

use super::expiration::Expiry;
pub const SEND_CW20_ACTION: &str = "SEND_CW20";
pub const SEND_NFT_ACTION: &str = "SEND_NFT";

#[cw_serde]
pub enum Asset {
    Cw20Token(AndrAddr),
    NativeToken(String),
}

impl Display for Asset {
    fn fmt(&self, f: &mut Formatter) -> StdResult {
        match self {
            Asset::NativeToken(addr) => f.write_str(&format!("native:{addr}")),
            Asset::Cw20Token(addr) => f.write_str(&format!("cw20:{addr}")),
        }
    }
}

impl Asset {
    pub fn get_verified_asset(
        &self,
        deps: DepsMut,
        env: Env,
    ) -> Result<(String, bool), ContractError> {
        match self {
            Asset::Cw20Token(cw20_token) => {
                let cw20_token = cw20_token.get_raw_address(&deps.as_ref())?;
                let token_info_query: TokenInfoResponse =
                    deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: cw20_token.to_string(),
                        msg: to_json_binary(&Cw20QueryMsg::TokenInfo {})?,
                    }))?;
                ensure!(
                    !token_info_query.total_supply.is_zero(),
                    ContractError::InvalidZeroAmount {}
                );
                let valid_cw20_auction = ADOContract::default()
                    .is_permissioned(deps, env, SEND_CW20_ACTION, cw20_token.clone())
                    .is_ok();
                ensure!(
                    valid_cw20_auction,
                    ContractError::InvalidFunds {
                        msg: format!("Non-permissioned CW20 asset '{}' set as denom.", cw20_token)
                    }
                );
                Ok((cw20_token.to_string(), true))
            }
            Asset::NativeToken(native) => {
                validate_native_denom(deps.as_ref(), native.clone())?;
                Ok((native.to_string(), false))
            }
        }
    }
    pub fn transfer(
        &self,
        deps: &Deps,
        to_address: impl Into<String>,
        amount: Uint128,
    ) -> Result<SubMsg, ContractError> {
        let to_address: String = to_address.into();

        Ok(match self {
            Asset::NativeToken(denom) => SubMsg::new(BankMsg::Send {
                to_address,
                amount: vec![coin(amount.u128(), denom)],
            }),
            Asset::Cw20Token(denom) => {
                let denom = denom.get_raw_address(deps)?;
                let transfer_msg = Cw20ExecuteMsg::Transfer {
                    recipient: to_address,
                    amount,
                };
                let wasm_msg = wasm_execute(denom, &transfer_msg, vec![])?;
                SubMsg::new(wasm_msg)
            }
        })
    }

    pub fn burn(&self, deps: &Deps, amount: Uint128) -> Result<SubMsg, ContractError> {
        Ok(match self {
            Asset::NativeToken(denom) => SubMsg::new(BankMsg::Burn {
                amount: vec![coin(amount.u128(), denom)],
            }),
            Asset::Cw20Token(denom) => {
                let denom = denom.get_raw_address(deps)?;
                let burn_msg = Cw20ExecuteMsg::Burn { amount };
                let wasm_msg = wasm_execute(denom, &burn_msg, vec![])?;
                SubMsg::new(wasm_msg)
            }
        })
    }
}

pub fn validate_native_denom(deps: Deps, denom: String) -> Result<(), ContractError> {
    let potential_supply = deps.querier.query_supply(denom.clone())?;
    let non_empty_denom = !denom.is_empty();
    let non_zero_supply = !potential_supply.amount.is_zero();
    ensure!(
        non_empty_denom && non_zero_supply,
        ContractError::InvalidAsset { asset: denom }
    );

    Ok(())
}

#[cw_serde]
pub struct AuthorizedAddressesResponse {
    pub addresses: Vec<String>,
}

pub fn authorize_addresses(
    deps: &mut DepsMut,
    action: &str,
    addresses: Vec<AndrAddr>,
) -> Result<(), ContractError> {
    if !addresses.is_empty() {
        ADOContract::default().permission_action(action, deps.storage)?;
    }

    for address in addresses {
        let addr = address.get_raw_address(&deps.as_ref())?;
        ADOContract::set_permission(
            deps.storage,
            action,
            addr.to_string(),
            Permission::Local(LocalPermission::whitelisted(None, None)),
        )?;
    }
    Ok(())
}

pub fn execute_authorize_contract(
    deps: DepsMut,
    info: MessageInfo,
    action: PermissionAction,
    address: AndrAddr,
    expiration: Option<Expiry>,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    ensure!(
        contract.is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let permission = expiration.map_or(
        Permission::Local(LocalPermission::whitelisted(None, None)),
        |expiration| Permission::Local(LocalPermission::whitelisted(None, Some(expiration))),
    );

    ADOContract::set_permission(
        deps.storage,
        action.as_str(),
        address.to_string(),
        permission.clone(),
    )?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "authorize_contract"),
        attr("address", address),
        attr("permission", permission.to_string()),
    ]))
}

pub fn execute_deauthorize_contract(
    deps: DepsMut,
    info: MessageInfo,
    action: PermissionAction,
    address: AndrAddr,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    ensure!(
        contract.is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let raw_address = address.get_raw_address(&deps.as_ref())?;

    ADOContract::remove_permission(deps.storage, action.as_str(), raw_address.to_string())?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "deauthorize_contract"),
        attr("address", raw_address),
        attr("deauthorized_action", action.as_str()),
    ]))
}

#[cw_serde]
pub enum PermissionAction {
    SendCw20,
    SendNft,
}

impl PermissionAction {
    pub fn as_str(&self) -> &str {
        match self {
            PermissionAction::SendCw20 => SEND_CW20_ACTION,
            PermissionAction::SendNft => SEND_NFT_ACTION,
        }
    }
}

impl TryFrom<String> for PermissionAction {
    type Error = ContractError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            SEND_CW20_ACTION => Ok(PermissionAction::SendCw20),
            SEND_NFT_ACTION => Ok(PermissionAction::SendNft),
            _ => Err(ContractError::InvalidAction { action: value }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_action() {
        // Test as_str() method
        assert_eq!(PermissionAction::SendCw20.as_str(), SEND_CW20_ACTION);
        assert_eq!(PermissionAction::SendNft.as_str(), SEND_NFT_ACTION);
        // Test TryFrom<String> implementation
        assert_eq!(
            PermissionAction::try_from(SEND_CW20_ACTION.to_string()),
            Ok(PermissionAction::SendCw20)
        );
        assert_eq!(
            PermissionAction::try_from(SEND_NFT_ACTION.to_string()),
            Ok(PermissionAction::SendNft)
        );

        // Test invalid action
        let invalid_action = "INVALID_ACTION".to_string();
        assert_eq!(
            PermissionAction::try_from(invalid_action.clone()),
            Err(ContractError::InvalidAction {
                action: invalid_action
            })
        );
    }
}
