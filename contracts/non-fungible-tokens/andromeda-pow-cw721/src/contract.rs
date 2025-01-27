#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError};

use andromeda_non_fungible_tokens::pow_cw721::{ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::{
    ado_base::{
        permissioning::{LocalPermission, Permission},
        InstantiateMsg as BaseInstantiateMsg, MigrateMsg,
    },
    ado_contract::ADOContract,
    common::{context::ExecuteContext, encode_binary},
    error::ContractError,
};

use crate::execute::handle_execute;
use crate::query::{query_linked_cw721_address, query_pow_nft};
use crate::state::LINKED_CW721_ADDRESS;

const CONTRACT_NAME: &str = "crates.io:andromeda-pow-cw721";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const MINT_POW_NFT_ACTION: &str = "MINT_POW_NFT";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let inst_resp = ADOContract::default().instantiate(
        deps.storage,
        env.clone(),
        deps.api,
        &deps.querier,
        info,
        BaseInstantiateMsg {
            ado_type: CONTRACT_NAME.to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            kernel_address: msg.kernel_address,
            owner: msg.owner,
        },
    )?;

    LINKED_CW721_ADDRESS.save(deps.storage, &msg.linked_cw721_address)?;

    // Set mint PoW NFT action permission
    if let Some(authorized_origin_minter_addresses) = msg.authorized_origin_minter_addresses {
        if !authorized_origin_minter_addresses.is_empty() {
            ADOContract::default().permission_action(MINT_POW_NFT_ACTION, deps.storage)?;
        }

        for origin_minter_address in authorized_origin_minter_addresses {
            let addr = origin_minter_address.get_raw_address(&deps.as_ref())?;
            ADOContract::set_permission(
                deps.storage,
                MINT_POW_NFT_ACTION,
                addr,
                Permission::Local(LocalPermission::Whitelisted(None)),
            )?;
        }
    }

    Ok(inst_resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let ctx = ExecuteContext::new(deps, info, env);
    match msg {
        ExecuteMsg::AMPReceive(pkt) => {
            ADOContract::default().execute_amp_receive(ctx, pkt, handle_execute)
        }
        _ => handle_execute(ctx, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetPowNFT { token_id } => encode_binary(&query_pow_nft(deps, token_id)?),
        QueryMsg::GetLinkedCw721Address {} => encode_binary(&query_linked_cw721_address(deps)?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.result.is_err() {
        return Err(ContractError::Std(StdError::generic_err(
            msg.result.unwrap_err(),
        )));
    }

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, CONTRACT_NAME, CONTRACT_VERSION)
}
