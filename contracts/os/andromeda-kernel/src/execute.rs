use crate::ibc::{get_counterparty_denom, PACKET_LIFETIME};
use andromeda_std::ado_contract::ADOContract;
use andromeda_std::amp::addresses::AndrAddr;
use andromeda_std::amp::messages::{AMPCtx, AMPMsg, AMPPkt, CrossChainHop};
use andromeda_std::amp::{ADO_DB_KEY, VFS_KEY};
use andromeda_std::common::context::ExecuteContext;
use andromeda_std::common::has_coins_merged;
use andromeda_std::common::reply::ReplyId;
use andromeda_std::error::ContractError;
use andromeda_std::os::aos_querier::AOSQuerier;
#[cfg(not(target_arch = "wasm32"))]
use andromeda_std::os::ibc_registry::path_to_hops;
use andromeda_std::os::kernel::{ChannelInfo, IbcExecuteMsg, Ics20PacketInfo, InternalMsg};
use andromeda_std::os::vfs::vfs_resolve_symlink;
use cosmwasm_std::{
    attr, ensure, from_json, to_json_binary, BankMsg, Binary, Coin, ContractInfoResponse,
    CosmosMsg, DepsMut, Env, IbcMsg, MessageInfo, Response, StdAck, StdError, SubMsg, WasmMsg,
};

use crate::query;
use crate::state::{
    ADO_OWNER, CHAIN_TO_CHANNEL, CHANNEL_TO_CHAIN, CHANNEL_TO_EXECUTE_MSG, CURR_CHAIN,
    ENV_VARIABLES, IBC_FUND_RECOVERY, KERNEL_ADDRESSES, PENDING_MSG_AND_FUNDS, TRIGGER_KEY,
};

pub fn send(ctx: ExecuteContext, message: AMPMsg) -> Result<Response, ContractError> {
    ensure!(
        has_coins_merged(ctx.info.funds.as_slice(), message.funds.as_slice()),
        ContractError::InsufficientFunds {}
    );
    let res = MsgHandler(message).handle(ctx.deps, ctx.info, ctx.env, ctx.amp_ctx, 0)?;
    Ok(res)
}

pub fn trigger_relay(
    ctx: ExecuteContext,
    packet_sequence: u64,
    channel_id: String,
    packet_ack_msg: Binary,
) -> Result<Response, ContractError> {
    //TODO Only the authorized address to handle replies can call this function
    ensure!(
        ctx.info.sender == KERNEL_ADDRESSES.load(ctx.deps.storage, TRIGGER_KEY)?,
        ContractError::Unauthorized {}
    );
    let ics20_packet_info = CHANNEL_TO_EXECUTE_MSG
        .load(ctx.deps.storage, (channel_id.clone(), packet_sequence))
        .expect("No packet found for channel_id and sequence");

    let chain =
        ics20_packet_info
            .recipient
            .get_chain()
            .ok_or_else(|| ContractError::InvalidPacket {
                error: Some("Chain not provided".to_string()),
            })?;

    let channel_info = CHAIN_TO_CHANNEL
        .may_load(ctx.deps.storage, chain)?
        .ok_or_else(|| ContractError::InvalidPacket {
            error: Some(format!("Channel not found for chain {}", chain)),
        })?;
    let ack: StdAck = from_json(packet_ack_msg)?;

    match ack {
        StdAck::Success(_) => handle_ibc_transfer_funds_reply(
            ctx.deps,
            ctx.info,
            ctx.env,
            ctx.amp_ctx,
            packet_sequence,
            channel_info,
            ics20_packet_info,
        ),
        // This means that the funds have been returned to the contract, time to return the funds to the original sender
        StdAck::Error(_) => {
            let refund_msg = CosmosMsg::Bank(BankMsg::Send {
                to_address: ics20_packet_info.sender.clone(),
                amount: vec![ics20_packet_info.funds.clone()],
            });
            Ok(Response::default()
                .add_message(refund_msg)
                .add_attribute("action", "relay_packet")
                .add_attribute("relay_outcome", "refund")
                .add_attribute("relay_sequence", packet_sequence.to_string())
                .add_attribute("relay_channel", channel_id)
                .add_attribute("relay_chain", chain)
                .add_attribute("recipient", ics20_packet_info.sender)
                .add_attribute("amount", ics20_packet_info.funds.to_string()))
        }
    }
}

fn handle_ibc_transfer_funds_reply(
    deps: DepsMut,
    _info: MessageInfo,
    env: Env,
    _ctx: Option<AMPPkt>,
    sequence: u64,
    channel_info: ChannelInfo,
    ics20_packet_info: Ics20PacketInfo,
) -> Result<Response, ContractError> {
    let ics20_packet_info = ics20_packet_info.clone();
    let chain =
        &ics20_packet_info
            .recipient
            .get_chain()
            .ok_or_else(|| ContractError::InvalidPacket {
                error: Some("Chain not provided in recipient".to_string()),
            })?;

    let channel = channel_info
        .direct_channel_id
        .ok_or_else(|| ContractError::InvalidPacket {
            error: Some(format!("Direct channel not found for chain {}", chain)),
        })?;

    // TODO: We should send denom info to the counterparty chain with amp message
    let (counterparty_denom, _counterparty_denom_info) = get_counterparty_denom(
        &deps.as_ref(),
        &ics20_packet_info.funds.denom,
        &ics20_packet_info.channel,
    )?;
    #[allow(unused_assignments, unused_mut)]
    let mut adjusted_funds = Coin::new(
        ics20_packet_info.funds.amount.u128(),
        counterparty_denom.clone(),
    );

    // Funds are not correctly hashed when using cw-orchestrator so instead we construct the denom manually
    #[cfg(not(target_arch = "wasm32"))]
    if counterparty_denom.starts_with("ibc/") {
        let hops = path_to_hops(_counterparty_denom_info.path)?;
        // cw-orch doesn't correctly hash the denom so we need to manually construct it
        let adjusted_path = hops
            .iter()
            .map(|hop| hop.channel_id.clone())
            .collect::<Vec<String>>()
            .join("/");

        adjusted_funds = Coin::new(
            ics20_packet_info.funds.amount.u128(),
            format!(
                "ibc/{}/{}",
                adjusted_path, _counterparty_denom_info.base_denom
            ),
        );
    }

    let mut ctx = AMPCtx::new(
        ics20_packet_info.sender.clone(),
        env.contract.address,
        0,
        None,
    );

    // Add the orginal sender's username if it exists
    let potential_username = ctx.try_add_origin_username(
        &deps.querier,
        &KERNEL_ADDRESSES.load(deps.storage, VFS_KEY)?,
    );

    // Create a new hop to be appended to the context
    let hop = CrossChainHop {
        username: potential_username.as_ref().map(AndrAddr::from_string),
        address: ics20_packet_info.sender.clone(),
        from_chain: CURR_CHAIN.load(deps.storage)?,
        to_chain: chain.to_string(),
        funds: vec![adjusted_funds.clone()],
        channel: channel.clone(),
    };

    // Add the new hop to the context
    ctx.add_hop(hop);

    let kernel_msg = IbcExecuteMsg::SendMessageWithFunds {
        recipient: AndrAddr::from_string(ics20_packet_info.recipient.clone().get_raw_path()),
        message: ics20_packet_info.message,
        funds: adjusted_funds,
        original_sender: ics20_packet_info.sender,
        original_sender_username: potential_username.map(AndrAddr::from_string),
        previous_hops: ctx.previous_hops,
    };
    let msg = IbcMsg::SendPacket {
        channel_id: channel.clone(),
        data: to_json_binary(&kernel_msg)?,
        timeout: env.block.time.plus_seconds(PACKET_LIFETIME).into(),
    };

    Ok(Response::default()
        .add_message(CosmosMsg::Ibc(msg))
        .add_attribute("action", "relay_packet")
        .add_attribute("relay_outcome", "success")
        .add_attribute("relay_sequence", sequence.to_string())
        .add_attribute("relay_channel", ics20_packet_info.channel)
        .add_attribute("relay_chain", chain.to_string())
        .add_attribute("receiving_kernel_address", channel_info.kernel_address))
}

pub fn amp_receive(
    deps: &mut DepsMut,
    info: MessageInfo,
    env: Env,
    packet: AMPPkt,
) -> Result<Response, ContractError> {
    // Only verified ADOs can access this function
    ensure!(
        info.sender == env.contract.address
            || query::verify_address(deps.as_ref(), info.sender.to_string())?.verify_address,
        ContractError::Unauthorized {}
    );
    ensure!(
        packet.ctx.id == 0,
        ContractError::InvalidPacket {
            error: Some("Packet ID cannot be provided from outside the Kernel".into())
        }
    );

    let mut res = Response::default();
    ensure!(
        !packet.messages.is_empty(),
        ContractError::InvalidPacket {
            error: Some("No messages supplied".to_string())
        }
    );

    let msg = packet.messages.first().unwrap();
    let mut handler = MsgHandler::new(msg.clone());
    let msg_res = handler.handle(
        deps.branch(),
        info.clone(),
        env.clone(),
        Some(packet.clone()),
        0,
    )?;

    res.messages.extend_from_slice(&msg_res.messages);
    res.attributes.extend_from_slice(&msg_res.attributes);
    res.events.extend_from_slice(&msg_res.events);

    let mut new_pkt = AMPPkt::from_ctx(Some(packet.clone()), env.contract.address.to_string());

    for (idx, message) in packet.messages.iter().enumerate() {
        if idx == 0 {
            continue;
        }
        new_pkt = new_pkt.add_message(message.clone());
    }

    if !new_pkt.messages.is_empty() {
        let new_funds = new_pkt
            .messages
            .iter()
            .flat_map(|m| m.funds.clone())
            .collect::<Vec<Coin>>();

        let new_pkt_msg =
            new_pkt.to_sub_msg(env.contract.address.to_string(), Some(new_funds), 0)?;
        res.messages.extend_from_slice(&[new_pkt_msg]);
    }

    let message_funds = packet
        .messages
        .iter()
        .flat_map(|m| m.funds.clone())
        .collect::<Vec<Coin>>();
    ensure!(
        has_coins_merged(info.funds.as_slice(), message_funds.as_slice()),
        ContractError::InsufficientFunds {}
    );

    Ok(res.add_attribute("action", "handle_amp_packet"))
}

pub fn upsert_key_address(
    execute_ctx: ExecuteContext,
    key: String,
    value: String,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    ensure!(
        contract.is_contract_owner(execute_ctx.deps.storage, execute_ctx.info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    // Updates to new value
    if KERNEL_ADDRESSES.has(execute_ctx.deps.storage, &key) {
        KERNEL_ADDRESSES.remove(execute_ctx.deps.storage, &key)
    }

    KERNEL_ADDRESSES.save(
        execute_ctx.deps.storage,
        &key,
        &execute_ctx.deps.api.addr_validate(&value)?,
    )?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "upsert_key_address"),
        attr("key", key),
        attr("value", value),
    ]))
}

pub fn create(
    execute_ctx: ExecuteContext,
    ado_type: String,
    msg: Binary,
    owner: Option<AndrAddr>,
    chain: Option<String>,
) -> Result<Response, ContractError> {
    // If chain is provided an owner must be provided
    ensure!(
        chain.is_none() || owner.is_some(),
        ContractError::Unauthorized {}
    );
    if let Some(chain) = chain {
        let cross_chain_components_enabled = ENV_VARIABLES
            .may_load(execute_ctx.deps.storage, "cross_chain_components_enabled")?
            .unwrap_or("false".to_string());
        ensure!(
            cross_chain_components_enabled == "true",
            ContractError::CrossChainComponentsCurrentlyDisabled {}
        );

        let channel_info = if let Some(channel_info) =
            CHAIN_TO_CHANNEL.may_load(execute_ctx.deps.storage, &chain)?
        {
            Ok::<ChannelInfo, ContractError>(channel_info)
        } else {
            return Err(ContractError::InvalidPacket {
                error: Some(format!("Channel not found for chain {chain}")),
            });
        }?;
        let kernel_msg = IbcExecuteMsg::CreateADO {
            instantiation_msg: msg.clone(),
            owner: owner.clone().unwrap(),
            ado_type: ado_type.clone(),
        };
        let ibc_msg = IbcMsg::SendPacket {
            channel_id: channel_info.direct_channel_id.clone().unwrap(),
            data: to_json_binary(&kernel_msg)?,
            timeout: execute_ctx
                .env
                .block
                .time
                .plus_seconds(PACKET_LIFETIME)
                .into(),
        };
        Ok(Response::default()
            .add_message(ibc_msg)
            .add_attributes(vec![
                attr("action", "execute_create"),
                attr("ado_type", ado_type),
                attr("owner", owner.unwrap().to_string()),
                attr("chain", chain),
                attr("receiving_kernel_address", channel_info.kernel_address),
                attr("msg", msg.to_string()),
            ]))
    } else {
        let vfs_addr = KERNEL_ADDRESSES.load(execute_ctx.deps.storage, VFS_KEY)?;
        let adodb_addr = KERNEL_ADDRESSES.load(execute_ctx.deps.storage, ADO_DB_KEY)?;

        let ado_owner = owner.unwrap_or(AndrAddr::from_string(execute_ctx.info.sender.to_string()));
        let owner_addr =
            ado_owner.get_raw_address_from_vfs(&execute_ctx.deps.as_ref(), vfs_addr)?;
        let code_id =
            AOSQuerier::code_id_getter(&execute_ctx.deps.querier, &adodb_addr, &ado_type)?;
        let wasm_msg = WasmMsg::Instantiate {
            admin: Some(owner_addr.to_string()),
            code_id,
            msg,
            funds: vec![],
            label: format!("ADO:{ado_type}"),
        };
        let sub_msg = SubMsg::reply_always(wasm_msg, ReplyId::CreateADO.repr());

        ADO_OWNER.save(execute_ctx.deps.storage, &owner_addr)?;

        Ok(Response::new()
            .add_submessage(sub_msg)
            .add_attribute("action", "execute_create")
            .add_attribute("ado_type", ado_type)
            .add_attribute("owner", ado_owner.to_string()))
    }
}

pub fn internal(ctx: ExecuteContext, msg: InternalMsg) -> Result<Response, ContractError> {
    match msg {
        InternalMsg::RegisterUserCrossChain {
            username,
            address,
            chain,
        } => register_user_cross_chain(ctx, chain, username, address),
    }
}

pub fn register_user_cross_chain(
    execute_ctx: ExecuteContext,
    chain: String,
    username: String,
    address: String,
) -> Result<Response, ContractError> {
    let vfs = KERNEL_ADDRESSES.load(execute_ctx.deps.storage, VFS_KEY)?;
    ensure!(
        execute_ctx.info.sender == vfs,
        ContractError::Unauthorized {}
    );
    let channel_info =
        if let Some(channel_info) = CHAIN_TO_CHANNEL.may_load(execute_ctx.deps.storage, &chain)? {
            Ok::<ChannelInfo, ContractError>(channel_info)
        } else {
            return Err(ContractError::InvalidPacket {
                error: Some(format!("Channel not found for chain {chain}")),
            });
        }?;
    let kernel_msg = IbcExecuteMsg::RegisterUsername {
        username: username.clone(),
        address: address.clone(),
    };
    let channel_id = if let Some(direct_channel_id) = channel_info.direct_channel_id {
        Ok::<String, ContractError>(direct_channel_id)
    } else {
        return Err(ContractError::InvalidPacket {
            error: Some(format!("Channel not found for chain {chain}")),
        });
    }?;
    let ibc_msg = IbcMsg::SendPacket {
        channel_id,
        data: to_json_binary(&kernel_msg)?,
        timeout: execute_ctx
            .env
            .block
            .time
            .plus_seconds(PACKET_LIFETIME)
            .into(),
    };

    Ok(Response::default()
        .add_attributes(vec![
            attr("action", "register_user_cross_chain"),
            attr("username", username),
            attr("address", address),
            attr("chain", chain),
            attr("receiving_kernel_address", channel_info.kernel_address),
        ])
        .add_message(ibc_msg))
}

pub fn assign_channels(
    execute_ctx: ExecuteContext,
    ics20_channel_id: Option<String>,
    direct_channel_id: Option<String>,
    chain: String,
    kernel_address: String,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    ensure!(
        contract.is_contract_owner(execute_ctx.deps.storage, execute_ctx.info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let mut channel_info = CHAIN_TO_CHANNEL
        .load(execute_ctx.deps.storage, &chain)
        .unwrap_or_default();
    channel_info.kernel_address = kernel_address;
    if let Some(channel) = direct_channel_id {
        // Remove old direct channel to chain if it exists
        if let Some(direct_channel_id) = channel_info.direct_channel_id {
            CHANNEL_TO_CHAIN.remove(execute_ctx.deps.storage, &direct_channel_id);
        }
        CHANNEL_TO_CHAIN.save(execute_ctx.deps.storage, &channel, &chain)?;
        channel_info.direct_channel_id = Some(channel);
    }
    if let Some(channel) = ics20_channel_id {
        // Remove old ics20 channel to chain if it exists
        if let Some(ics20_channel_id) = channel_info.ics20_channel_id {
            CHANNEL_TO_CHAIN.remove(execute_ctx.deps.storage, &ics20_channel_id);
        }
        CHANNEL_TO_CHAIN.save(execute_ctx.deps.storage, &channel, &chain)?;
        channel_info.ics20_channel_id = Some(channel);
    }
    CHAIN_TO_CHANNEL.save(execute_ctx.deps.storage, &chain, &channel_info)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "assign_channel"),
        attr(
            "ics20_channel_id",
            channel_info.ics20_channel_id.unwrap_or("None".to_string()),
        ),
        attr(
            "direct_channel_id",
            channel_info.direct_channel_id.unwrap_or("None".to_string()),
        ),
        attr("chain", chain),
        attr("kernel_address", channel_info.kernel_address),
    ]))
}

pub fn recover(execute_ctx: ExecuteContext) -> Result<Response, ContractError> {
    let recoveries = IBC_FUND_RECOVERY
        .load(execute_ctx.deps.storage, &execute_ctx.info.sender)
        .unwrap_or_default();
    IBC_FUND_RECOVERY.remove(execute_ctx.deps.storage, &execute_ctx.info.sender);
    ensure!(
        !recoveries.is_empty(),
        ContractError::Std(StdError::generic_err("No recoveries found"))
    );

    let bank_msg = BankMsg::Send {
        to_address: execute_ctx.info.sender.to_string(),
        amount: recoveries,
    };
    let sub_msg = SubMsg::reply_always(bank_msg, ReplyId::Recovery.repr());

    Ok(Response::default()
        .add_attribute("action", "recover")
        .add_submessage(sub_msg))
}

pub fn update_chain_name(
    execute_ctx: ExecuteContext,
    chain_name: String,
) -> Result<Response, ContractError> {
    // Only owner can update CURR_CHAIN
    let contract = ADOContract::default();
    ensure!(
        contract.is_contract_owner(execute_ctx.deps.storage, execute_ctx.info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    // Update CURR_CHAIN
    CURR_CHAIN.save(execute_ctx.deps.storage, &chain_name)?;

    Ok(Response::default()
        .add_attribute("action", "update_chain_name")
        .add_attribute("sender", execute_ctx.info.sender.as_str())
        .add_attribute("chain_name", chain_name))
}

pub fn set_env(
    execute_ctx: ExecuteContext,
    variable: String,
    value: String,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    ensure!(
        contract.is_contract_owner(execute_ctx.deps.storage, execute_ctx.info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    ensure!(
        !variable.is_empty(),
        ContractError::InvalidEnvironmentVariable {
            msg: "Environment variable name cannot be empty".to_string()
        }
    );

    ensure!(
        variable
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_'),
        ContractError::InvalidEnvironmentVariable {
            msg:
                "Environment variable name can only contain alphanumeric characters and underscores"
                    .to_string()
        }
    );

    ensure!(
        variable.len() <= 100,
        ContractError::InvalidEnvironmentVariable {
            msg: "Environment variable name length exceeds the maximum allowed length of 100 characters".to_string()
        }
    );

    ensure!(
        !value.is_empty(),
        ContractError::InvalidEnvironmentVariable {
            msg: "Environment variable value cannot be empty".to_string()
        }
    );

    ensure!(
        value.len() <= 100,
        ContractError::InvalidEnvironmentVariable {
            msg: "Environment variable value length exceeds the maximum allowed length of 100 characters".to_string()
        }
    );

    ENV_VARIABLES.save(
        execute_ctx.deps.storage,
        &variable.to_ascii_uppercase(),
        &value,
    )?;
    Ok(Response::default()
        .add_attribute("action", "set_env")
        .add_attribute("variable", variable)
        .add_attribute("value", value))
}

pub fn unset_env(execute_ctx: ExecuteContext, variable: String) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    ensure!(
        contract.is_contract_owner(execute_ctx.deps.storage, execute_ctx.info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    ensure!(
        ENV_VARIABLES
            .may_load(execute_ctx.deps.storage, &variable.to_ascii_uppercase())?
            .is_some(),
        ContractError::EnvironmentVariableNotFound { variable }
    );
    ENV_VARIABLES.remove(execute_ctx.deps.storage, &variable.to_ascii_uppercase());
    Ok(Response::default()
        .add_attribute("action", "unset_env")
        .add_attribute("variable", variable))
}

/// Handles a given AMP message and returns a response
///
/// Separated due to common functionality across multiple messages
#[derive(Clone)]
pub struct MsgHandler(AMPMsg);

impl MsgHandler {
    pub fn new(msg: AMPMsg) -> Self {
        Self(msg)
    }

    fn message(&self) -> &AMPMsg {
        &self.0
    }

    fn update_recipient(&mut self, recipient: AndrAddr) -> Self {
        self.0.recipient = recipient;
        self.clone()
    }

    #[inline]
    pub fn handle(
        &mut self,
        deps: DepsMut,
        info: MessageInfo,
        env: Env,
        ctx: Option<AMPPkt>,
        sequence: u64,
    ) -> Result<Response, ContractError> {
        let resolved_recipient = if self.message().recipient.is_vfs_path() {
            let vfs_address = KERNEL_ADDRESSES.load(deps.storage, VFS_KEY)?;
            vfs_resolve_symlink(
                self.message().recipient.clone(),
                vfs_address.to_string(),
                &deps.querier,
            )?
        } else {
            self.message().recipient.clone()
        };
        self.update_recipient(resolved_recipient);
        let protocol = self.message().recipient.get_protocol();
        match protocol {
            Some("ibc") => self.handle_ibc(deps, info, env, ctx, sequence),
            _ => self.handle_local(deps, info, env, ctx.map(|ctx| ctx.ctx), sequence),
        }
    }

    /**
    Handles a local AMP Message, that is a message that has no defined protocol in its recipient VFS path. There are two different situations for a local message that are defined by the binary message provided.
    Situation 1 is that the message provided is empty or `Binary::default` in which case the message must be a `BankMsg::Send` message and the funds must be provided.
    Situation 2 is that the message has a provided binary and must be a `WasmMsg::Execute` message.

    In both situations the sender can define the funds that are being attached to the message.
    */
    pub fn handle_local(
        &self,
        deps: DepsMut,
        info: MessageInfo,
        _env: Env,
        ctx: Option<AMPCtx>,
        sequence: u64,
    ) -> Result<Response, ContractError> {
        let mut res = Response::default();

        let original_msg = self.message();
        let AMPMsg {
            message,
            recipient,
            funds,
            config,
            ..
        } = original_msg;

        let recipient_addr = recipient.get_raw_address(&deps.as_ref())?;

        let adodb_addr = KERNEL_ADDRESSES.load(deps.storage, ADO_DB_KEY)?;

        if Binary::default() == message.clone() {
            ensure!(
                !funds.is_empty(),
                ContractError::InvalidPacket {
                    error: Some("No message or funds supplied".to_string())
                }
            );

            let sub_msg = BankMsg::Send {
                to_address: recipient_addr.to_string(),
                amount: funds.clone(),
            };

            let mut attrs = vec![];
            for (idx, fund) in funds.iter().enumerate() {
                attrs.push(attr(format!("funds:{sequence}:{idx}"), fund.to_string()));
            }
            attrs.push(attr(format!("recipient:{sequence}"), recipient_addr));

            res = res
                .add_submessage(SubMsg::reply_on_error(
                    CosmosMsg::Bank(sub_msg),
                    ReplyId::AMPMsg.repr(),
                ))
                .add_attributes(attrs);
        } else {
            let origin = if let Some(amp_ctx) = ctx {
                amp_ctx.get_origin()
            } else {
                info.sender.to_string()
            };
            let previous_sender = info.sender.to_string();

            let ContractInfoResponse {
                code_id: recipient_code_id,
                ..
            } = deps
                .querier
                .query_wasm_contract_info(recipient_addr.clone())
                .ok()
                .ok_or(ContractError::InvalidPacket {
                    error: Some("Recipient is not a contract".to_string()),
                })?;

            let sub_msg = if config.direct
                || AOSQuerier::ado_type_getter(&deps.querier, &adodb_addr, recipient_code_id)?
                    .is_none()
            {
                self.message()
                    .generate_sub_msg_direct(recipient_addr.clone(), ReplyId::AMPMsg.repr())
            } else {
                let amp_msg =
                    AMPMsg::new(recipient_addr.clone(), message.clone(), Some(funds.clone()))
                        .with_config(config.clone());

                let new_packet = AMPPkt::new(origin, previous_sender, vec![amp_msg]);

                new_packet.to_sub_msg(
                    recipient_addr.clone(),
                    Some(funds.clone()),
                    ReplyId::AMPMsg.repr(),
                )?
            };

            res = res
                .add_submessage(sub_msg)
                .add_attributes(vec![attr(format!("recipient:{sequence}"), recipient_addr)]);
        }
        Ok(res)
    }

    /**
    Handles an IBC AMP Message. An IBC AMP Message is defined by adding the `ibc://<chain>` protocol definition to the start of the VFS path.
    The `chain` is the chain ID of the destination chain and an appropriate channel must be present for the given chain.

    The VFS path has its protocol stripped and the message is passed via ibc-hooks to the kernel on the receiving chain. The kernel on the receiving chain will receive the message as if it was sent from the local chain and will act accordingly.
    */
    fn handle_ibc(
        &self,
        deps: DepsMut,
        info: MessageInfo,
        env: Env,
        ctx: Option<AMPPkt>,
        sequence: u64,
    ) -> Result<Response, ContractError> {
        if let Some(chain) = self.message().recipient.get_chain() {
            let channel_info =
                if let Some(channel_info) = CHAIN_TO_CHANNEL.may_load(deps.storage, chain)? {
                    Ok::<ChannelInfo, ContractError>(channel_info)
                } else {
                    return Err(ContractError::InvalidPacket {
                        error: Some(format!("Channel not found for chain {chain}")),
                    });
                }?;
            if !self.message().funds.is_empty() {
                self.handle_ibc_transfer_funds(deps, info, env, ctx, sequence, channel_info)
            } else {
                self.handle_ibc_direct(deps, info, env, ctx, sequence, channel_info)
            }
        } else {
            Err(ContractError::InvalidPacket {
                error: Some("Chain not provided".to_string()),
            })
        }
    }

    fn handle_ibc_direct(
        &self,
        deps: DepsMut,
        info: MessageInfo,
        env: Env,
        ctx: Option<AMPPkt>,
        sequence: u64,
        channel_info: ChannelInfo,
    ) -> Result<Response, ContractError> {
        let AMPMsg {
            recipient,
            message,
            funds,
            ..
        } = self.message();
        ensure!(
            !Binary::default().eq(message),
            ContractError::InvalidPacket {
                error: Some("Cannot send an empty message without funds via IBC".to_string())
            }
        );
        let destination_chain = recipient.get_chain().unwrap();
        let channel = if let Some(direct_channel) = channel_info.direct_channel_id {
            Ok::<String, ContractError>(direct_channel)
        } else {
            return Err(ContractError::InvalidPacket {
                error: Some(format!("Channel not found for chain {destination_chain}")),
            });
        }?;
        let vfs_address = KERNEL_ADDRESSES.load(deps.storage, VFS_KEY).unwrap();
        let current_chain = CURR_CHAIN.load(deps.storage)?;

        let ctx = ctx.map_or_else(
            || {
                let amp_msg = AMPMsg::new(recipient.clone().get_raw_path(), message.clone(), None);
                let mut ctx = AMPCtx::new(info.sender, env.contract.address, 0, None);

                // Add the orginal sender's username if it exists
                let potential_username = ctx.try_add_origin_username(&deps.querier, &vfs_address);

                // Create a new hop to be appended to the context
                let hop = CrossChainHop {
                    username: potential_username.map(AndrAddr::from_string),
                    address: ctx.get_origin(),
                    from_chain: current_chain.to_string(),
                    to_chain: destination_chain.to_string(),
                    funds: funds.to_vec(),
                    channel: channel.clone(),
                };

                // Add the new hop to the context
                ctx.add_hop(hop);

                AMPPkt::new_with_ctx(ctx, vec![amp_msg])
            },
            |mut ctx| {
                // Add the orginal sender's username if it exists
                let potential_username =
                    ctx.ctx.try_add_origin_username(&deps.querier, &vfs_address);

                // Create a new hop to be appended to the context
                let hop = CrossChainHop {
                    username: potential_username.map(AndrAddr::from_string),
                    address: ctx.ctx.get_origin(),
                    from_chain: current_chain.to_string(),
                    to_chain: destination_chain.to_string(),
                    funds: funds.to_vec(),
                    channel: channel.clone(),
                };

                // Add the new hop to the context
                ctx.ctx.add_hop(hop);

                // Remove the recipient's prepended chain reference
                ctx.messages[0].recipient =
                    AndrAddr::from_string(recipient.clone().get_raw_path().to_string());
                ctx
            },
        );

        let kernel_msg = IbcExecuteMsg::SendMessage { amp_packet: ctx };

        let msg = IbcMsg::SendPacket {
            channel_id: channel.clone(),
            data: to_json_binary(&kernel_msg)?,
            timeout: env.block.time.plus_seconds(PACKET_LIFETIME).into(),
        };

        Ok(Response::default()
            .add_attribute(format!("method:{sequence}"), "execute_send_message")
            .add_attribute(format!("channel:{sequence}"), channel)
            .add_attribute("receiving_kernel_address:{}", channel_info.kernel_address)
            .add_attribute("chain:{}", destination_chain)
            .add_message(msg))
    }

    fn handle_ibc_transfer_funds(
        &self,
        deps: DepsMut,
        info: MessageInfo,
        env: Env,
        _ctx: Option<AMPPkt>,
        sequence: u64,
        channel_info: ChannelInfo,
    ) -> Result<Response, ContractError> {
        let AMPMsg {
            recipient,
            message,
            funds,
            ..
        } = self.message();
        let chain = recipient.get_chain().unwrap();
        let channel = if let Some(ics20_channel) = channel_info.ics20_channel_id {
            Ok::<String, ContractError>(ics20_channel)
        } else {
            return Err(ContractError::InvalidPacket {
                error: Some(format!("Channel not found for chain {chain}")),
            });
        }?;
        deps.api.debug(&format!("info.funds: {:?}", info.funds));
        deps.api.debug(&format!("funds: {:?}", funds));
        ensure!(
            funds.len() == 1,
            ContractError::InvalidFunds {
                msg: "Number of funds should be exactly one".to_string()
            }
        );
        let coin = funds
            .first()
            .ok_or(ContractError::InvalidPacket {
                error: Some("Transfer funds must contain funds in the AMPMsg".to_string()),
            })?
            .clone();

        let msg = IbcMsg::Transfer {
            channel_id: channel.clone(),
            to_address: channel_info.kernel_address.clone(),
            amount: coin.clone(),
            timeout: env.block.time.plus_seconds(PACKET_LIFETIME).into(),
        };
        let mut resp = Response::default();

        // Save execute msg, to be loaded in the reply
        PENDING_MSG_AND_FUNDS.save(
            deps.storage,
            &Ics20PacketInfo {
                sender: info.sender.into_string(),
                recipient: recipient.clone(),
                message: message.clone(),
                funds: coin,
                channel: channel.clone(),
            },
        )?;
        resp = resp.add_submessage(SubMsg {
            id: ReplyId::IBCTransfer.repr(),
            msg: CosmosMsg::Ibc(msg),
            gas_limit: None,
            reply_on: cosmwasm_std::ReplyOn::Always,
        });
        Ok(resp
            .add_attribute(format!("method:{sequence}"), "execute_transfer_funds")
            .add_attribute(format!("channel:{sequence}"), channel)
            .add_attribute("receiving_kernel_address:{}", channel_info.kernel_address)
            .add_attribute("chain:{}", chain))
    }
}
