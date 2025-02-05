use crate::ado_contract::ADOContract;
use crate::common::encode_binary;
use crate::error::ContractError;
use crate::os::aos_querier::AOSQuerier;
use crate::os::kernel::ExecuteMsg as KernelExecuteMsg;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    to_json_binary, wasm_execute, Addr, Binary, Coin, ContractInfoResponse, CosmosMsg, Deps, Empty,
    MessageInfo, QuerierWrapper, QueryRequest, ReplyOn, SubMsg, WasmMsg, WasmQuery,
};

use super::addresses::AndrAddr;
use super::ADO_DB_KEY;

/// Exposed for ease of serialisation.
#[cw_serde]
pub enum ExecuteMsg {
    /// The common message enum to receive an AMP message within a contract.
    #[serde(rename = "amp_receive")]
    AMPReceive(AMPPkt),
}

#[cw_serde]
#[derive(Default)]
pub struct IBCConfig {
    pub recovery_addr: Option<AndrAddr>,
}

impl IBCConfig {
    #[inline]
    pub fn new(recovery_addr: Option<AndrAddr>) -> IBCConfig {
        IBCConfig { recovery_addr }
    }
}

/// The configuration of the message to be sent.
///
/// Used when a sub message is generated for the given AMP Msg (only used in the case of Wasm Messages).
#[cw_serde]
pub struct AMPMsgConfig {
    /// When the message should reply, defaults to Always
    pub reply_on: ReplyOn,
    /// Determines whether the operation should terminate or proceed upon a failed message
    pub exit_at_error: bool,
    /// An optional imposed gas limit for the message
    pub gas_limit: Option<u64>,
    /// Whether to send the message directly to the given recipient
    pub direct: bool,
    pub ibc_config: Option<IBCConfig>,
}

impl AMPMsgConfig {
    #[inline]
    pub fn new(
        reply_on: Option<ReplyOn>,
        exit_at_error: Option<bool>,
        gas_limit: Option<u64>,
        ibc_config: Option<IBCConfig>,
    ) -> AMPMsgConfig {
        AMPMsgConfig {
            reply_on: reply_on.unwrap_or(ReplyOn::Always),
            exit_at_error: exit_at_error.unwrap_or(true),
            gas_limit,
            direct: false,
            ibc_config,
        }
    }

    /// Converts the current AMP message to be a direct message to the given contract
    pub fn as_direct_msg(self) -> AMPMsgConfig {
        AMPMsgConfig {
            reply_on: self.reply_on,
            exit_at_error: self.exit_at_error,
            gas_limit: self.gas_limit,
            direct: true,
            ibc_config: self.ibc_config,
        }
    }
}

impl Default for AMPMsgConfig {
    #[inline]
    fn default() -> AMPMsgConfig {
        AMPMsgConfig {
            reply_on: ReplyOn::Always,
            exit_at_error: true,
            gas_limit: None,
            direct: false,
            ibc_config: None,
        }
    }
}

#[cw_serde]
/// This struct defines how the kernel parses and relays messages between ADOs
/// If the desired recipient is via IBC then namespacing must be employed
/// The attached message must be a binary encoded execute message for the receiving ADO
/// Funds can be attached for an individual message and will be attached accordingly
pub struct AMPMsg {
    /// The message recipient, can be a contract/wallet address or a namespaced URI
    pub recipient: AndrAddr,
    /// The message to be sent to the recipient
    pub message: Binary,
    /// Any funds to be attached to the message, defaults to an empty vector
    pub funds: Vec<Coin>,
    /// When the message should reply, defaults to Always
    pub config: AMPMsgConfig,
}

impl AMPMsg {
    /// Creates a new AMPMsg
    pub fn new(recipient: impl Into<String>, message: Binary, funds: Option<Vec<Coin>>) -> AMPMsg {
        AMPMsg {
            recipient: AndrAddr::from_string(recipient),
            message,
            funds: funds.unwrap_or_default(),
            config: AMPMsgConfig::default(),
        }
    }

    pub fn with_config(&self, config: AMPMsgConfig) -> AMPMsg {
        AMPMsg {
            recipient: self.recipient.clone(),
            message: self.message.clone(),
            funds: self.funds.clone(),
            config,
        }
    }

    /// Generates an AMPPkt containing the given AMPMsg
    pub fn generate_amp_pkt(
        &self,
        deps: &Deps,
        origin: impl Into<String>,
        previous_sender: impl Into<String>,
        id: u64,
    ) -> Result<SubMsg, ContractError> {
        let contract_addr = self.recipient.get_raw_address(deps)?;
        let pkt = AMPPkt::new(origin, previous_sender, vec![self.clone()]);
        let msg = to_json_binary(&ExecuteMsg::AMPReceive(pkt))?;
        Ok(SubMsg {
            id,
            reply_on: self.config.reply_on.clone(),
            gas_limit: self.config.gas_limit,
            msg: CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.into(),
                msg,
                funds: self.funds.to_vec(),
            }),
        })
    }

    pub fn generate_sub_msg_direct(&self, addr: Addr, id: u64) -> SubMsg<Empty> {
        SubMsg {
            id,
            reply_on: self.config.reply_on.clone(),
            gas_limit: self.config.gas_limit,
            msg: CosmosMsg::Wasm(wasm_execute(addr, &self.message, self.funds.to_vec()).unwrap()),
        }
    }

    pub fn to_ibc_hooks_memo(&self, contract_addr: String, callback_addr: String) -> String {
        #[derive(::serde::Serialize)]
        struct IbcHooksWasmMsg<T: ::serde::Serialize> {
            contract: String,
            msg: T,
        }
        #[derive(::serde::Serialize)]
        struct IbcHooksMsg<T: ::serde::Serialize> {
            wasm: IbcHooksWasmMsg<T>,
            ibc_callback: String,
        }
        let wasm_msg = IbcHooksWasmMsg {
            contract: contract_addr,
            msg: KernelExecuteMsg::Send {
                message: self.clone(),
            },
        };
        let msg = IbcHooksMsg {
            wasm: wasm_msg,
            ibc_callback: callback_addr,
        };

        serde_json_wasm::to_string(&msg).unwrap()
    }

    /// Adds an IBC recovery address to the message
    pub fn with_ibc_recovery(&self, recovery_addr: Option<AndrAddr>) -> AMPMsg {
        if let Some(ibc_config) = self.config.ibc_config.clone() {
            let mut ibc_config = ibc_config;
            ibc_config.recovery_addr = recovery_addr;
            let mut msg = self.clone();
            msg.config.ibc_config = Some(ibc_config);
            msg
        } else if let Some(recovery_addr) = recovery_addr {
            let ibc_config = Some(IBCConfig {
                recovery_addr: Some(recovery_addr),
            });
            let mut msg = self.clone();
            msg.config.ibc_config = ibc_config;
            msg
        } else {
            self.clone()
        }
    }
}

#[cw_serde]
pub struct AMPCtx {
    origin: String,
    origin_username: Option<AndrAddr>,
    pub previous_sender: String,
    pub id: u64,
    pub previous_hops: Vec<CrossChainHop>,
}

impl AMPCtx {
    #[inline]
    pub fn new(
        origin: impl Into<String>,
        previous_sender: impl Into<String>,
        id: u64,
        origin_username: Option<AndrAddr>,
    ) -> AMPCtx {
        AMPCtx {
            origin: origin.into(),
            origin_username,
            previous_sender: previous_sender.into(),
            id,
            previous_hops: vec![],
        }
    }

    /// Gets the original sender of a message
    pub fn get_origin(&self) -> String {
        self.origin.clone()
    }

    /// Gets the username of the original sender
    pub fn get_origin_username(&self) -> Option<AndrAddr> {
        self.origin_username.clone()
    }

    /// Gets the previous sender of a message
    pub fn get_previous_sender(&self) -> String {
        self.previous_sender.clone()
    }

    /// Adds a cross-chain hop to the context's previous hops
    pub fn add_hop(&mut self, hop: CrossChainHop) {
        self.previous_hops.push(hop);
    }

    /// Adds a username
    pub fn add_username(&mut self, username: AndrAddr) {
        self.origin_username = Some(username);
    }

    pub fn try_add_origin_username(
        &mut self,
        querier: &QuerierWrapper,
        vfs_address: &Addr,
    ) -> Option<String> {
        let origin = Addr::unchecked(self.get_origin());
        if let Ok(Some(username)) = AOSQuerier::get_username(querier, vfs_address, &origin) {
            if username != origin {
                self.add_username(AndrAddr::from_string(username.clone()));
                return Some(username);
            }
        }
        None
    }
}

#[cw_serde]
pub struct CrossChainHop {
    /// The username of the sender at this hop (if available)
    pub username: Option<AndrAddr>,
    /// The address of the sender at this hop
    pub address: String,
    /// The chain this hop originated from
    pub from_chain: String,
    /// The chain this hop was directed to
    pub to_chain: String,
    /// Any funds that were attached to this hop
    pub funds: Vec<Coin>,
    /// The IBC channels used for this hop
    pub channel: String,
}

#[cw_serde]
/// An Andromeda packet contains all message protocol related data, this is what is sent between ADOs when communicating
/// It contains an original sender, if used for authorisation the sender must be authorised
/// The previous sender is the one who sent the message
/// A packet may contain several messages which allows for message batching
pub struct AMPPkt {
    /// Any messages associated with the packet
    pub messages: Vec<AMPMsg>,
    pub ctx: AMPCtx,
}

impl AMPPkt {
    /// Creates a new AMP Packet
    pub fn new(
        origin: impl Into<String>,
        previous_sender: impl Into<String>,
        messages: Vec<AMPMsg>,
    ) -> AMPPkt {
        AMPPkt {
            messages,
            ctx: AMPCtx::new(origin, previous_sender, 0, None),
        }
    }

    /// Creates a new AMP Packet
    pub fn new_with_ctx(ctx: AMPCtx, messages: Vec<AMPMsg>) -> AMPPkt {
        AMPPkt { messages, ctx }
    }

    pub fn with_origin(&self, origin: impl Into<String>) -> AMPPkt {
        let mut pkt = self.clone();
        pkt.ctx.origin = origin.into();
        pkt
    }

    /// Adds a message to the current AMP Packet
    pub fn add_message(mut self, message: AMPMsg) -> Self {
        self.messages.push(message);
        self
    }

    /// Gets all unique recipients for messages
    pub fn get_unique_recipients(&self) -> Vec<String> {
        let mut recipients: Vec<String> = self
            .messages
            .iter()
            .cloned()
            .map(|msg| msg.recipient.to_string())
            .collect();
        recipients.sort_unstable();
        recipients.dedup();
        recipients
    }

    /// Gets all messages for a given recipient
    pub fn get_messages_for_recipient(&self, recipient: String) -> Vec<AMPMsg> {
        self.messages
            .iter()
            .filter(|msg| msg.recipient == recipient.clone())
            .cloned()
            .collect()
    }

    /// Used to verify that the sender of the AMPPkt is authorised to attach the given origin field.
    /// A sender is valid if:
    ///
    /// 1. The origin matches the sender
    /// 2. The sender is the kernel
    /// 3. The sender has a code ID stored within the ADODB (and as such is a valid ADO)
    ///
    /// If the sender is not valid, an error is returned
    pub fn verify_origin(&self, info: &MessageInfo, deps: &Deps) -> Result<(), ContractError> {
        let kernel_address = ADOContract::default().get_kernel_address(deps.storage)?;

        if (info.sender == self.ctx.origin && info.sender == self.ctx.previous_sender)
            || info.sender == kernel_address
        {
            Ok(())
        } else {
            let adodb_address: Addr =
                AOSQuerier::kernel_address_getter(&deps.querier, &kernel_address, ADO_DB_KEY)?;

            // Get the sender's Code ID
            let contract_info: ContractInfoResponse =
                deps.querier
                    .query(&QueryRequest::Wasm(WasmQuery::ContractInfo {
                        contract_addr: info.sender.to_string(),
                    }))?;

            let sender_code_id = contract_info.code_id;
            // We query the ADO type in the adodb, it will return an error if the sender's Code ID doesn't exist.
            AOSQuerier::verify_code_id(&deps.querier, &adodb_address, sender_code_id)
        }
    }

    ///Verifies the origin of the AMPPkt and returns the origin if it is valid
    pub fn get_verified_origin(
        &self,
        info: &MessageInfo,
        deps: &Deps,
    ) -> Result<String, ContractError> {
        let origin = self.ctx.get_origin();
        let res = self.verify_origin(info, deps);
        match res {
            Ok(_) => Ok(origin),
            Err(err) => Err(err),
        }
    }

    /// Generates a SubMsg to send the AMPPkt to the kernel
    pub fn to_sub_msg(
        &self,
        address: impl Into<String>,
        funds: Option<Vec<Coin>>,
        id: u64,
    ) -> Result<SubMsg, ContractError> {
        let sub_msg = SubMsg::reply_always(
            WasmMsg::Execute {
                contract_addr: address.into(),
                msg: encode_binary(&KernelExecuteMsg::AMPReceive(self.clone()))?,
                funds: funds.unwrap_or_default(),
            },
            id,
        );
        Ok(sub_msg)
    }

    ///  Attaches an ID to the current packet
    pub fn with_id(&self, id: u64) -> AMPPkt {
        let mut new = self.clone();
        new.ctx.id = id;
        new
    }

    /// Converts a given AMP Packet to an IBC Hook memo for use with Osmosis' IBC Hooks module
    pub fn to_ibc_hooks_memo(&self, contract_addr: String, callback_addr: String) -> String {
        #[derive(::serde::Serialize)]
        struct IbcHooksWasmMsg<T: ::serde::Serialize> {
            contract: String,
            msg: T,
        }
        #[derive(::serde::Serialize)]
        struct IbcHooksMsg<T: ::serde::Serialize> {
            wasm: IbcHooksWasmMsg<T>,
            ibc_callback: String,
        }
        let wasm_msg = IbcHooksWasmMsg {
            contract: contract_addr,
            msg: KernelExecuteMsg::AMPReceive(self.clone()),
        };
        let msg = IbcHooksMsg {
            wasm: wasm_msg,
            ibc_callback: callback_addr,
        };

        serde_json_wasm::to_string(&msg).unwrap()
    }

    /// Serializes the given AMP Packet to a JSON string
    pub fn to_json(&self) -> String {
        serde_json_wasm::to_string(&self).unwrap()
    }

    /// Generates an AMP Packet from context
    pub fn from_ctx(ctx: Option<AMPPkt>, current_address: String) -> Self {
        let mut ctx = if let Some(pkt) = ctx {
            pkt.ctx
        } else {
            AMPCtx::new(current_address.clone(), current_address.clone(), 0, None)
        };
        ctx.previous_sender = current_address;

        Self {
            messages: vec![],
            ctx,
        }
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_info};

    use crate::testing::mock_querier::{mock_dependencies_custom, INVALID_CONTRACT};

    use super::*;

    #[test]
    fn test_generate_amp_pkt() {
        let deps = mock_dependencies();
        let msg = AMPMsg::new("test", Binary::default(), None);

        let sub_msg = msg
            .generate_amp_pkt(&deps.as_ref(), "origin", "previoussender", 1)
            .unwrap();

        let expected_msg = ExecuteMsg::AMPReceive(AMPPkt::new(
            "origin",
            "previoussender",
            vec![AMPMsg::new("test", Binary::default(), None)],
        ));
        assert_eq!(sub_msg.id, 1);
        assert_eq!(sub_msg.reply_on, ReplyOn::Always);
        assert_eq!(sub_msg.gas_limit, None);
        assert_eq!(
            sub_msg.msg,
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "test".to_string(),
                msg: to_json_binary(&expected_msg).unwrap(),
                funds: vec![],
            })
        );
    }

    #[test]
    fn test_get_unique_recipients() {
        let msg = AMPMsg::new("test", Binary::default(), None);
        let msg2 = AMPMsg::new("test2", Binary::default(), None);

        let mut pkt = AMPPkt::new("origin", "previoussender", vec![msg, msg2]);

        let recipients = pkt.get_unique_recipients();
        assert_eq!(recipients.len(), 2);
        assert_eq!(recipients[0], "test".to_string());
        assert_eq!(recipients[1], "test2".to_string());

        pkt = pkt.add_message(AMPMsg::new("test", Binary::default(), None));
        let recipients = pkt.get_unique_recipients();
        assert_eq!(recipients.len(), 2);
        assert_eq!(recipients[0], "test".to_string());
        assert_eq!(recipients[1], "test2".to_string());
    }

    #[test]
    fn test_get_messages_for_recipient() {
        let msg = AMPMsg::new("test", Binary::default(), None);
        let msg2 = AMPMsg::new("test2", Binary::default(), None);

        let mut pkt = AMPPkt::new("origin", "previoussender", vec![msg, msg2]);

        let messages = pkt.get_messages_for_recipient("test".to_string());
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].recipient.to_string(), "test".to_string());

        let messages = pkt.get_messages_for_recipient("test2".to_string());
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].recipient.to_string(), "test2".to_string());

        pkt = pkt.add_message(AMPMsg::new("test", Binary::default(), None));
        let messages = pkt.get_messages_for_recipient("test".to_string());
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].recipient.to_string(), "test".to_string());
        assert_eq!(messages[1].recipient.to_string(), "test".to_string());
    }

    #[test]
    fn test_verify_origin() {
        let deps = mock_dependencies_custom(&[]);
        let msg = AMPMsg::new("test", Binary::default(), None);

        let pkt = AMPPkt::new("origin", "previoussender", vec![msg.clone()]);

        let info = mock_info("validaddress", &[]);
        let res = pkt.verify_origin(&info, &deps.as_ref());
        assert!(res.is_ok());

        let info = mock_info(INVALID_CONTRACT, &[]);
        let res = pkt.verify_origin(&info, &deps.as_ref());
        assert!(res.is_err());

        // Previous sender and origin set correctly
        let direct_pkt_valid = AMPPkt::new(INVALID_CONTRACT, INVALID_CONTRACT, vec![msg.clone()]);
        let res = direct_pkt_valid.verify_origin(&info, &deps.as_ref());
        assert!(res.is_ok());

        // Previous sender and origin set incorrectly
        let direct_pkt_invalid = AMPPkt::new(INVALID_CONTRACT, "notvalid", vec![msg]);
        let res = direct_pkt_invalid.verify_origin(&info, &deps.as_ref());
        assert!(res.is_err());
    }

    #[test]
    fn test_to_sub_msg() {
        let msg = AMPMsg::new("test", Binary::default(), None);

        let pkt = AMPPkt::new("origin", "previoussender", vec![msg.clone()]);

        let sub_msg = pkt.to_sub_msg("kernel", None, 1).unwrap();

        let expected_msg =
            ExecuteMsg::AMPReceive(AMPPkt::new("origin", "previoussender", vec![msg]));
        assert_eq!(sub_msg.id, 1);
        assert_eq!(sub_msg.reply_on, ReplyOn::Always);
        assert_eq!(sub_msg.gas_limit, None);
        assert_eq!(
            sub_msg.msg,
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "kernel".to_string(),
                msg: to_json_binary(&expected_msg).unwrap(),
                funds: vec![],
            })
        );
    }
    #[test]
    fn test_to_json() {
        let msg = AMPPkt::new("origin", "previoussender", vec![]);

        let memo = msg.to_json();
        assert_eq!(memo, "{\"messages\":[],\"ctx\":{\"origin\":\"origin\",\"origin_username\":null,\"previous_sender\":\"previoussender\",\"id\":0,\"previous_hops\":[]}}".to_string());
    }

    #[test]
    fn test_to_ibc_hooks_memo() {
        let msg = AMPPkt::new("origin", "previoussender", vec![]);
        let contract_addr = "contractaddr";
        let memo = msg.to_ibc_hooks_memo(contract_addr.to_string(), "callback".to_string());
        assert_eq!(memo, "{\"wasm\":{\"contract\":\"contractaddr\",\"msg\":{\"amp_receive\":{\"messages\":[],\"ctx\":{\"origin\":\"origin\",\"origin_username\":null,\"previous_sender\":\"previoussender\",\"id\":0,\"previous_hops\":[]}}}},\"ibc_callback\":\"callback\"}".to_string());
    }
}
