#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError};

use andromeda_accounts::fixed_multisig::{ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    ado_contract::ADOContract,
    common::{context::ExecuteContext, encode_binary},
    error::ContractError,
};
use cw2::set_contract_version;

use crate::execute::handle_execute;
use crate::query::{
    list_proposals, list_voters, list_votes, query_proposal, query_threshold, query_vote,
    query_voter, reverse_proposals,
};
use crate::state::{Config, CONFIG, VOTERS};

// version info for migration info
pub const CONTRACT_NAME: &str = "crates.io:andromeda-fixed-multisig";
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let resp = ADOContract::default().instantiate(
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

    // This contains functionality derived from the cw3-fixed-multisig contract.
    // Source: https://github.com/CosmWasm/cw-plus/blob/main/contracts/cw3-fixed-multisig
    // License: Apache-2.0

    if msg.voters.is_empty() {
        return Err(ContractError::CustomError {
            msg: "No voters".to_string(),
        });
    }
    let total_weight = msg.voters.iter().map(|v| v.weight).sum();

    msg.threshold.validate(total_weight)?;

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let cfg = Config {
        threshold: msg.threshold,
        total_weight,
        max_voting_period: msg.max_voting_period,
    };
    CONFIG.save(deps.storage, &cfg)?;

    // add all voters
    for voter in msg.voters.iter() {
        let key = deps
            .api
            .addr_validate(voter.addr.get_raw_address(&deps.as_ref())?.as_str())?;
        VOTERS.save(deps.storage, &key, &voter.weight)?;
    }

    Ok(resp)
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
        QueryMsg::Threshold {} => encode_binary(&query_threshold(deps)?),
        QueryMsg::Proposal { proposal_id } => {
            encode_binary(&query_proposal(deps, env, proposal_id)?)
        }
        QueryMsg::Vote { proposal_id, voter } => {
            encode_binary(&query_vote(deps, proposal_id, voter)?)
        }
        QueryMsg::ListProposals { start_after, limit } => {
            encode_binary(&list_proposals(deps, env, start_after, limit)?)
        }
        QueryMsg::ReverseProposals {
            start_before,
            limit,
        } => encode_binary(&reverse_proposals(deps, env, start_before, limit)?),
        QueryMsg::ListVotes {
            proposal_id,
            start_after,
            limit,
        } => encode_binary(&list_votes(deps, proposal_id, start_after, limit)?),
        QueryMsg::Voter { address } => encode_binary(&query_voter(deps, address)?),
        QueryMsg::ListVoters { start_after, limit } => {
            encode_binary(&list_voters(deps, start_after, limit)?)
        }
        _ => ADOContract::default().query(deps, env, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, env, CONTRACT_NAME, CONTRACT_VERSION)
}
