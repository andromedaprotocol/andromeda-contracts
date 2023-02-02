#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure, from_binary, to_binary, Addr, Api, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Storage, SubMsg, Uint128, WasmMsg,
};

use ado_base::ADOContract;
use andromeda_fungible_tokens::cw20::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use common::{
    ado_base::{hooks::AndromedaHook, AndromedaMsg, InstantiateMsg as BaseInstantiateMsg},
    error::ContractError,
    Funds,
};
use cw2::{get_contract_version, set_contract_version};
use cw20::{Cw20Coin, Cw20ExecuteMsg};
use cw20_base::{
    contract::{execute as execute_cw20, instantiate as cw20_instantiate, query as query_cw20},
    state::BALANCES,
};
use semver::Version;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-cw20";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let contract = ADOContract::default();
    let resp = contract.instantiate(
        deps.storage,
        env.clone(),
        deps.api,
        info.clone(),
        BaseInstantiateMsg {
            ado_type: "cw20".to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            operators: None,
            modules: msg.modules.clone(),
            primitive_contract: None,
            kernel_address: msg.clone().kernel_address,
        },
    )?;
    let cw20_resp = cw20_instantiate(deps, env, info, msg.into())?;

    Ok(resp
        .add_submessages(cw20_resp.messages)
        .add_attributes(cw20_resp.attributes))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();

    // Do this before the hooks get fired off to ensure that there are no errors from the app
    // address not being fully setup yet.
    if let ExecuteMsg::AndrReceive(andr_msg) = msg.clone() {
        if let AndromedaMsg::UpdateAppContract { address } = andr_msg {
            return contract.execute_update_app_contract(deps, info, address, None);
        } else if let AndromedaMsg::UpdateOwner { address } = andr_msg {
            return contract.execute_update_owner(deps, info, address);
        }
    }

    contract.module_hook::<Response>(
        deps.storage,
        deps.api,
        deps.querier,
        AndromedaHook::OnExecute {
            sender: info.sender.to_string(),
            payload: to_binary(&msg)?,
        },
    )?;

    match msg {
        ExecuteMsg::Transfer { recipient, amount } => {
            execute_transfer(deps, env, info, recipient, amount)
        }
        ExecuteMsg::Burn { amount } => execute_burn(deps, env, info, amount),
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => execute_send(deps, env, info, contract, amount, msg),
        ExecuteMsg::Mint { recipient, amount } => execute_mint(deps, env, info, recipient, amount),
        ExecuteMsg::AndrReceive(msg) => contract.execute(deps, env, info, msg, execute),
        _ => Ok(execute_cw20(deps, env, info, msg.into())?),
    }
}

fn execute_transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let (msgs, events, remainder) = ADOContract::default().on_funds_transfer(
        deps.storage,
        deps.api,
        &deps.querier,
        info.sender.to_string(),
        Funds::Cw20(Cw20Coin {
            address: env.contract.address.to_string(),
            amount,
        }),
        to_binary(&ExecuteMsg::Transfer {
            amount,
            recipient: recipient.clone(),
        })?,
    )?;

    let remaining_amount = match remainder {
        Funds::Native(..) => amount, //What do we do in the case that the rates returns remaining amount as native funds?
        Funds::Cw20(coin) => coin.amount,
    };

    let mut resp = filter_out_cw20_messages(msgs, deps.storage, deps.api, &info.sender)?;

    // Continue with standard cw20 operation
    let cw20_resp = execute_cw20(
        deps,
        env,
        info,
        Cw20ExecuteMsg::Transfer {
            recipient,
            amount: remaining_amount,
        },
    )?;
    resp = resp.add_attributes(cw20_resp.attributes).add_events(events);
    Ok(resp)
}

fn transfer_tokens(
    storage: &mut dyn Storage,
    sender: &Addr,
    recipient: &Addr,
    amount: Uint128,
) -> Result<(), ContractError> {
    BALANCES.update(
        storage,
        sender,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    BALANCES.update(
        storage,
        recipient,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;
    Ok(())
}

fn execute_burn(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    Ok(execute_cw20(
        deps,
        env,
        info,
        Cw20ExecuteMsg::Burn { amount },
    )?)
}

fn execute_send(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    contract: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    let (msgs, events, remainder) = ADOContract::default().on_funds_transfer(
        deps.storage,
        deps.api,
        &deps.querier,
        info.sender.to_string(),
        Funds::Cw20(Cw20Coin {
            address: env.contract.address.to_string(),
            amount,
        }),
        to_binary(&ExecuteMsg::Send {
            amount,
            contract: contract.clone(),
            msg: msg.clone(),
        })?,
    )?;

    let remaining_amount = match remainder {
        Funds::Native(..) => amount, //What do we do in the case that the rates returns remaining amount as native funds?
        Funds::Cw20(coin) => coin.amount,
    };

    let mut resp = filter_out_cw20_messages(msgs, deps.storage, deps.api, &info.sender)?;

    let cw20_resp = execute_cw20(
        deps,
        env,
        info,
        Cw20ExecuteMsg::Send {
            contract,
            amount: remaining_amount,
            msg,
        },
    )?;
    resp = resp
        .add_attributes(cw20_resp.attributes)
        .add_events(events)
        .add_submessages(cw20_resp.messages);

    Ok(resp)
}

fn execute_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    Ok(execute_cw20(
        deps,
        env,
        info,
        Cw20ExecuteMsg::Mint { recipient, amount },
    )?)
}

fn filter_out_cw20_messages(
    msgs: Vec<SubMsg>,
    storage: &mut dyn Storage,
    api: &dyn Api,
    sender: &Addr,
) -> Result<Response, ContractError> {
    let mut resp: Response = Response::new();
    // Filter through payment messages to extract cw20 transfer messages to avoid looping
    for sub_msg in msgs {
        // Transfer messages are CosmosMsg::Wasm type
        if let CosmosMsg::Wasm(WasmMsg::Execute { msg: exec_msg, .. }) = sub_msg.msg.clone() {
            // If binary deserializes to a Cw20ExecuteMsg check the message type
            if let Ok(Cw20ExecuteMsg::Transfer { recipient, amount }) =
                from_binary::<Cw20ExecuteMsg>(&exec_msg)
            {
                transfer_tokens(storage, sender, &api.addr_validate(&recipient)?, amount)?;
            } else {
                resp = resp.add_submessage(sub_msg);
            }
        } else {
            resp = resp.add_submessage(sub_msg);
        }
    }
    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // New version
    let version: Version = CONTRACT_VERSION.parse().map_err(from_semver)?;

    // Old version
    let stored = get_contract_version(deps.storage)?;
    let storage_version: Version = stored.version.parse().map_err(from_semver)?;

    let contract = ADOContract::default();

    ensure!(
        stored.contract == CONTRACT_NAME,
        ContractError::CannotMigrate {
            previous_contract: stored.contract,
        }
    );

    // New version has to be newer/greater than the old version
    ensure!(
        storage_version < version,
        ContractError::CannotMigrate {
            previous_contract: stored.version,
        }
    );

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Update the ADOContract's version
    contract.execute_update_version(deps)?;

    Ok(Response::default())
}

fn from_semver(err: semver::Error) -> StdError {
    StdError::generic_err(format!("Semver: {err}"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::AndrQuery(msg) => ADOContract::default().query(deps, env, msg, query),
        _ => Ok(query_cw20(deps, env, msg.into())?),
    }
}
