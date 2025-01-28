use andromeda_std::ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg};
use andromeda_std::ado_contract::ADOContract;
use andromeda_std::common::context::ExecuteContext;
use andromeda_std::common::encode_binary;
use andromeda_std::common::reply::ReplyId;
use andromeda_std::error::ContractError;

use andromeda_std::os::kernel::{ExecuteMsg, InstantiateMsg, QueryMsg};
use cosmwasm_std::{
    entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError,
};

use crate::execute::handle_receive_cw20;
use crate::ibc::{IBCLifecycleComplete, SudoMsg};
use crate::reply::{
    on_reply_create_ado, on_reply_ibc_hooks_packet_send, on_reply_ibc_transfer,
    on_reply_refund_ibc_transfer_with_msg,
};
use crate::state::CURR_CHAIN;
use crate::{execute, query, sudo};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-kernel";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    CURR_CHAIN.save(deps.storage, &msg.chain_name)?;

    ADOContract::default().instantiate(
        deps.storage,
        env.clone(),
        deps.api,
        &deps.querier,
        info,
        BaseInstantiateMsg {
            ado_type: CONTRACT_NAME.to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            kernel_address: env.contract.address.to_string(),
            owner: msg.owner,
        },
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(mut deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.result.is_err() {
        match ReplyId::from_repr(msg.id) {
            Some(ReplyId::IBCTransferWithMsg) => {
                return on_reply_refund_ibc_transfer_with_msg(
                    deps.branch(),
                    env.clone(),
                    msg.clone(),
                );
            }
            _ => {
                return Err(ContractError::Std(StdError::generic_err(format!(
                    "{}:{}",
                    msg.id,
                    msg.result.unwrap_err()
                ))))
            }
        }
    }

    match ReplyId::from_repr(msg.id) {
        Some(ReplyId::CreateADO) => on_reply_create_ado(deps, env, msg),
        Some(ReplyId::IBCHooksPacketSend) => on_reply_ibc_hooks_packet_send(deps, msg),
        Some(ReplyId::IBCTransfer) => on_reply_ibc_transfer(deps, env, msg),
        _ => Ok(Response::default()),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let mut execute_env = ExecuteContext::new(deps, info, env);

    match msg {
        ExecuteMsg::AMPReceive(packet) => execute::amp_receive(
            &mut execute_env.deps,
            execute_env.info,
            execute_env.env,
            packet,
        ),
        ExecuteMsg::Receive(msg) => handle_receive_cw20(execute_env, msg),
        ExecuteMsg::Send { message } => execute::send(execute_env, message),
        ExecuteMsg::TriggerRelay {
            packet_sequence,
            channel_id,
            packet_ack,
        } => execute::trigger_relay(execute_env, packet_sequence, channel_id, packet_ack),
        ExecuteMsg::UpsertKeyAddress { key, value } => {
            execute::upsert_key_address(execute_env, key, value)
        }
        ExecuteMsg::Create {
            ado_type,
            msg,
            owner,
            chain,
        } => execute::create(execute_env, ado_type, msg, owner, chain),
        ExecuteMsg::AssignChannels {
            ics20_channel_id,
            direct_channel_id,
            chain,
            kernel_address,
        } => execute::assign_channels(
            execute_env,
            ics20_channel_id,
            direct_channel_id,
            chain,
            kernel_address,
        ),
        ExecuteMsg::Recover {} => execute::recover(execute_env),
        ExecuteMsg::UpdateChainName { chain_name } => {
            execute::update_chain_name(execute_env, chain_name)
        }
        ExecuteMsg::SetEnv { variable, value } => execute::set_env(execute_env, variable, value),
        ExecuteMsg::UnsetEnv { variable } => execute::unset_env(execute_env, variable),
        ExecuteMsg::Internal(msg) => execute::internal(execute_env, msg),
        ExecuteMsg::Ownership(ownership_message) => ADOContract::default().execute_ownership(
            execute_env.deps,
            execute_env.env,
            execute_env.info,
            ownership_message,
        ),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, _env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    match msg {
        SudoMsg::IBCLifecycleComplete(IBCLifecycleComplete::IBCAck {
            channel,
            sequence,
            ack,
            success,
        }) => sudo::ibc_lifecycle::receive_ack(deps, channel, sequence, ack, success),
        SudoMsg::IBCLifecycleComplete(IBCLifecycleComplete::IBCTimeout { channel, sequence }) => {
            sudo::ibc_lifecycle::receive_timeout(deps, channel, sequence)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, env, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::KeyAddress { key } => encode_binary(&query::key_address(deps, key)?),
        QueryMsg::VerifyAddress { address } => {
            encode_binary(&query::verify_address(deps, address)?)
        }
        QueryMsg::ChannelInfo { chain } => encode_binary(&query::channel_info(deps, chain)?),
        QueryMsg::Recoveries { addr } => encode_binary(&query::recoveries(deps, addr)?),
        QueryMsg::ChainName {} => encode_binary(&query::chain_name(deps)?),
        // Base queries
        QueryMsg::Version {} => encode_binary(&ADOContract::default().query_version(deps)?),
        QueryMsg::AdoType {} => encode_binary(&ADOContract::default().query_type(deps)?),
        QueryMsg::Owner {} => encode_binary(&ADOContract::default().query_contract_owner(deps)?),
        QueryMsg::ChainNameByChannel { channel } => {
            encode_binary(&query::chain_name_by_channel(deps, channel)?)
        }
        QueryMsg::PendingPackets { channel_id } => {
            encode_binary(&query::pending_packets(deps, channel_id)?)
        }
        QueryMsg::GetEnv { variable } => encode_binary(&query::get_env(deps, variable)?),
    }
}
