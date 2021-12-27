#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, QueryRequest,
    Response, StdResult, Uint128, WasmMsg, WasmQuery,
};
use cw2::set_contract_version;
use serde::de::DeserializeOwned;

use crate::state::{Config, CONFIG};
use andromeda_protocol::{
    error::ContractError,
    mirror_wrapped_cdp::{
        ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MirrorGovQueryMsg,
        MirrorMintQueryMsg, MirrorStakingQueryMsg, QueryMsg,
    },
    ownership::{execute_update_owner, is_contract_owner, query_contract_owner, CONTRACT_OWNER},
    require,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use mirror_protocol::{
    gov::{
        ConfigResponse as GovConfigResponse, PollResponse, PollsResponse, SharesResponse,
        StakerResponse, StateResponse as GovStateResponse, VotersResponse, VotersResponseItem,
    },
    mint::{
        AssetConfigResponse, ConfigResponse as MintConfigResponse, NextPositionIdxResponse,
        PositionResponse, PositionsResponse,
    },
    staking::{ConfigResponse as StakingConfigResponse, PoolInfoResponse, RewardInfoResponse},
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda_mirror_wrapped_cdp";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        mirror_mint_contract: deps.api.addr_validate(&msg.mirror_mint_contract)?,
        mirror_staking_contract: deps.api.addr_validate(&msg.mirror_staking_contract)?,
        mirror_gov_contract: deps.api.addr_validate(&msg.mirror_gov_contract)?,
    };
    CONFIG.save(deps.storage, &config)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    CONTRACT_OWNER.save(deps.storage, &info.sender)?;
    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, info, msg),
        ExecuteMsg::MirrorMintExecuteMsg(msg) => execute_mirror_msg(
            info,
            config.mirror_mint_contract.to_string(),
            to_binary(&msg)?,
        ),
        ExecuteMsg::MirrorStakingExecuteMsg(msg) => execute_mirror_msg(
            info,
            config.mirror_staking_contract.to_string(),
            to_binary(&msg)?,
        ),
        ExecuteMsg::MirrorGovExecuteMsg(msg) => execute_mirror_msg(
            info,
            config.mirror_gov_contract.to_string(),
            to_binary(&msg)?,
        ),
        ExecuteMsg::UpdateOwner { address } => execute_update_owner(deps, info, address),
        ExecuteMsg::UpdateConfig {
            mirror_mint_contract,
            mirror_staking_contract,
            mirror_gov_contract,
        } => execute_update_config(
            deps,
            info,
            mirror_mint_contract,
            mirror_staking_contract,
            mirror_gov_contract,
        ),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::MirrorMintCw20HookMsg(msg) => execute_mirror_cw20_msg(
            info,
            cw20_msg.sender.to_string(),
            cw20_msg.amount,
            config.mirror_mint_contract.to_string(),
            to_binary(&msg)?,
        ),
        Cw20HookMsg::MirrorStakingCw20HookMsg(msg) => execute_mirror_cw20_msg(
            info,
            cw20_msg.sender.to_string(),
            cw20_msg.amount,
            config.mirror_staking_contract.to_string(),
            to_binary(&msg)?,
        ),
        Cw20HookMsg::MirrorGovCw20HookMsg(msg) => execute_mirror_cw20_msg(
            info,
            cw20_msg.sender.to_string(),
            cw20_msg.amount,
            config.mirror_gov_contract.to_string(),
            to_binary(&msg)?,
        ),
    }
}

pub fn execute_mirror_cw20_msg(
    info: MessageInfo,
    token_addr: String,
    amount: Uint128,
    contract_addr: String,
    msg_binary: Binary,
) -> Result<Response, ContractError> {
    let msg = Cw20ExecuteMsg::Send {
        contract: token_addr,
        amount,
        msg: msg_binary,
    };
    execute_mirror_msg(info, contract_addr, to_binary(&msg)?)
}

pub fn execute_mirror_msg(
    info: MessageInfo,
    contract_addr: String,
    msg_binary: Binary,
) -> Result<Response, ContractError> {
    let execute_msg = WasmMsg::Execute {
        contract_addr,
        funds: info.funds,
        msg: msg_binary,
    };
    Ok(Response::new().add_messages(vec![CosmosMsg::Wasm(execute_msg)]))
}

pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    mirror_mint_contract: Option<String>,
    mirror_staking_contract: Option<String>,
    mirror_gov_contract: Option<String>,
) -> Result<Response, ContractError> {
    require(
        is_contract_owner(deps.storage, info.sender.to_string())?,
        ContractError::Unauthorized {},
    )?;
    let mut config = CONFIG.load(deps.storage)?;
    if let Some(mirror_mint_contract) = mirror_mint_contract {
        config.mirror_mint_contract = deps.api.addr_validate(&mirror_mint_contract)?;
    }
    if let Some(mirror_staking_contract) = mirror_staking_contract {
        config.mirror_staking_contract = deps.api.addr_validate(&mirror_staking_contract)?;
    }
    if let Some(mirror_gov_contract) = mirror_gov_contract {
        config.mirror_gov_contract = deps.api.addr_validate(&mirror_gov_contract)?;
    }
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ContractOwner {} => to_binary(&query_contract_owner(deps)?),
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::MirrorMintQueryMsg(msg) => query_mirror_mint(deps, msg),
        QueryMsg::MirrorStakingQueryMsg(msg) => query_mirror_staking(deps, msg),
        QueryMsg::MirrorGovQueryMsg(msg) => query_mirror_gov(deps, msg),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        mirror_mint_contract: config.mirror_mint_contract.to_string(),
        mirror_staking_contract: config.mirror_staking_contract.to_string(),
        mirror_gov_contract: config.mirror_gov_contract.to_string(),
    })
}

pub fn query_mirror_mint(deps: Deps, msg: MirrorMintQueryMsg) -> StdResult<Binary> {
    let contract_addr = CONFIG.load(deps.storage)?.mirror_mint_contract.to_string();
    match msg {
        MirrorMintQueryMsg::Config {} => to_binary(&query_mirror_msg::<MintConfigResponse>(
            deps,
            contract_addr,
            to_binary(&msg)?,
        )?),
        MirrorMintQueryMsg::AssetConfig { .. } => {
            to_binary(&query_mirror_msg::<AssetConfigResponse>(
                deps,
                contract_addr,
                to_binary(&msg)?,
            )?)
        }
        MirrorMintQueryMsg::Position { .. } => to_binary(&query_mirror_msg::<PositionResponse>(
            deps,
            contract_addr,
            to_binary(&msg)?,
        )?),
        MirrorMintQueryMsg::Positions { .. } => to_binary(&query_mirror_msg::<PositionsResponse>(
            deps,
            contract_addr,
            to_binary(&msg)?,
        )?),
        MirrorMintQueryMsg::NextPositionIdx {} => {
            to_binary(&query_mirror_msg::<NextPositionIdxResponse>(
                deps,
                contract_addr,
                to_binary(&msg)?,
            )?)
        }
    }
}

pub fn query_mirror_staking(deps: Deps, msg: MirrorStakingQueryMsg) -> StdResult<Binary> {
    let contract_addr = CONFIG
        .load(deps.storage)?
        .mirror_staking_contract
        .to_string();
    match msg {
        MirrorStakingQueryMsg::Config {} => to_binary(&query_mirror_msg::<StakingConfigResponse>(
            deps,
            contract_addr,
            to_binary(&msg)?,
        )?),
        MirrorStakingQueryMsg::PoolInfo { .. } => to_binary(&query_mirror_msg::<PoolInfoResponse>(
            deps,
            contract_addr,
            to_binary(&msg)?,
        )?),
        MirrorStakingQueryMsg::RewardInfo { .. } => {
            to_binary(&query_mirror_msg::<RewardInfoResponse>(
                deps,
                contract_addr,
                to_binary(&msg)?,
            )?)
        }
    }
}

pub fn query_mirror_gov(deps: Deps, msg: MirrorGovQueryMsg) -> StdResult<Binary> {
    let contract_addr = CONFIG.load(deps.storage)?.mirror_gov_contract.to_string();
    match msg {
        MirrorGovQueryMsg::Config {} => to_binary(&query_mirror_msg::<GovConfigResponse>(
            deps,
            contract_addr,
            to_binary(&msg)?,
        )?),
        MirrorGovQueryMsg::State {} => to_binary(&query_mirror_msg::<GovStateResponse>(
            deps,
            contract_addr,
            to_binary(&msg)?,
        )?),
        MirrorGovQueryMsg::Staker { .. } => to_binary(&query_mirror_msg::<StakerResponse>(
            deps,
            contract_addr,
            to_binary(&msg)?,
        )?),
        MirrorGovQueryMsg::Poll { .. } => to_binary(&query_mirror_msg::<PollResponse>(
            deps,
            contract_addr,
            to_binary(&msg)?,
        )?),
        MirrorGovQueryMsg::Polls { .. } => to_binary(&query_mirror_msg::<PollsResponse>(
            deps,
            contract_addr,
            to_binary(&msg)?,
        )?),
        MirrorGovQueryMsg::Voter { .. } => to_binary(&query_mirror_msg::<VotersResponseItem>(
            deps,
            contract_addr,
            to_binary(&msg)?,
        )?),
        MirrorGovQueryMsg::Voters { .. } => to_binary(&query_mirror_msg::<VotersResponse>(
            deps,
            contract_addr,
            to_binary(&msg)?,
        )?),
        MirrorGovQueryMsg::Shares { .. } => to_binary(&query_mirror_msg::<SharesResponse>(
            deps,
            contract_addr,
            to_binary(&msg)?,
        )?),
    }
}

pub fn query_mirror_msg<T: DeserializeOwned>(
    deps: Deps,
    contract_addr: String,
    msg_binary: Binary,
) -> StdResult<T> {
    let query_msg = WasmQuery::Smart {
        contract_addr,
        msg: msg_binary,
    };
    deps.querier.query(&QueryRequest::Wasm(query_msg))
}
