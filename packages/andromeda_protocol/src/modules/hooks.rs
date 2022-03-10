use cosmwasm_std::{Coin, DepsMut, Env, Event, MessageInfo, SubMsg};
use cw721::Expiration;

use crate::error::ContractError;

pub const ATTR_DESC: &str = "description";
pub const ATTR_PAYMENT: &str = "payment";
pub const ATTR_DEDUCTED: &str = "deducted";

#[derive(Debug, PartialEq)]
/// A struct used by module hooks to return any `Event` structs or `SubMsg` structs related to the module's hook.
/// May be combined using `self.add_resp`.
pub struct HookResponse {
    /// A vector of [SubMsg](https://docs.rs/cosmwasm-std/0.16.0/cosmwasm_std/struct.SubMsg.html) structs related to the hook.
    /// May be used to send payments or any other related messages.
    pub msgs: Vec<SubMsg>,
    /// A vector of CosmWasm [Event](https://docs.rs/cosmwasm-std/0.16.0/cosmwasm_std/struct.Event.html) structs.
    /// Used to define any events that the hook generated.
    pub events: Vec<Event>,
}

impl HookResponse {
    /// Instantiates a default `HookResponse`
    pub fn default() -> Self {
        HookResponse {
            msgs: vec![],
            events: vec![],
        }
    }
    /// Adds a CosmWasm [Event](https://docs.rs/cosmwasm-std/0.16.0/cosmwasm_std/struct.Event.html) to the `HookResponse`
    pub fn add_event(mut self, event: Event) -> Self {
        self.events.push(event);
        self
    }
    /// Adds a CosmWasm [SubMsg](https://docs.rs/cosmwasm-std/0.16.0/cosmwasm_std/struct.SubMsg.html) to the `HookResponse`
    pub fn add_message(mut self, message: SubMsg) -> Self {
        self.msgs.push(message);
        self
    }
    /// Concatenates another `HookResponse`
    pub fn add_resp(mut self, resp: HookResponse) -> Self {
        for event in resp.events {
            self.events.push(event);
        }
        for msg in resp.msgs {
            self.msgs.push(msg)
        }
        self
    }
}

/// An attribute struct used for any events that involve a payment
pub struct PaymentAttribute {
    /// The amount paid
    pub amount: Coin,
    /// The address the payment was made to
    pub receiver: String,
}

impl ToString for PaymentAttribute {
    fn to_string(&self) -> String {
        format!("{}<{}", self.receiver, self.amount)
    }
}

/// Hooks implemented by every module.
/// A hook is related to a contract event; either a message or a particular condition.
/// The hook is called when the condition is met or the message is received.
/// A hook may return a `HookResponse` which contains any `SubMsg` or `Event` structs generated by the hook.
/// Each hook is provided data related to the event.
pub trait MessageHooks {
    /// Called when an `InstantiateMsg` is received
    fn on_instantiate(
        &self,
        _deps: &DepsMut,
        _info: MessageInfo,
        _env: Env,
    ) -> Result<HookResponse, ContractError> {
        Ok(HookResponse::default())
    }
    /// Called when any `ExecuteMsg` is received
    fn on_execute(
        &self,
        _deps: &DepsMut,
        _info: MessageInfo,
        _env: Env,
    ) -> Result<HookResponse, ContractError> {
        Ok(HookResponse::default())
    }
    /// Called when an `ExecuteMsg::Mint` message is received
    fn on_mint(
        &self,
        _deps: &DepsMut,
        _info: MessageInfo,
        _env: Env,
        _token_id: String,
    ) -> Result<HookResponse, ContractError> {
        Ok(HookResponse::default())
    }
    /// Called when an `ExecuteMsg::Transfer` message is received
    fn on_transfer(
        &self,
        _deps: &DepsMut,
        _info: MessageInfo,
        _env: Env,
        _recipient: String,
        _token_id: String,
    ) -> Result<HookResponse, ContractError> {
        Ok(HookResponse::default())
    }
    /// Called when an `ExecuteMsg::Send` message is received
    fn on_send(
        &self,
        _deps: &DepsMut,
        _info: MessageInfo,
        _env: Env,
        _contract: String,
        _token_id: String,
    ) -> Result<HookResponse, ContractError> {
        Ok(HookResponse::default())
    }
    /// Called when an `ExecuteMsg::Approve` message is received
    fn on_approve(
        &self,
        _deps: &DepsMut,
        _info: MessageInfo,
        _env: Env,
        _spender: String,
        _token_id: String,
        _expires: Option<Expiration>,
    ) -> Result<HookResponse, ContractError> {
        Ok(HookResponse::default())
    }
    /// Called when an `ExecuteMsg::Revoke` message is received
    fn on_revoke(
        &self,
        _deps: &DepsMut,
        _info: MessageInfo,
        _env: Env,
        _sender: String,
        _token_id: String,
    ) -> Result<HookResponse, ContractError> {
        Ok(HookResponse::default())
    }
    /// Called when an `ExecuteMsg::ApproveAll` message is received
    fn on_approve_all(
        &self,
        _deps: &DepsMut,
        _info: MessageInfo,
        _env: Env,
        _operator: String,
        _expires: Option<Expiration>,
    ) -> Result<HookResponse, ContractError> {
        Ok(HookResponse::default())
    }
    /// Called when an `ExecuteMsg::RevokeAll` message is received
    fn on_revoke_all(
        &self,
        _deps: &DepsMut,
        _info: MessageInfo,
        _env: Env,
        _operator: String,
    ) -> Result<HookResponse, ContractError> {
        Ok(HookResponse::default())
    }
    /// Called when an `ExecuteMsg::TransferAgreement` message is received
    fn on_transfer_agreement(
        &self,
        _deps: &DepsMut,
        _info: MessageInfo,
        _env: Env,
        _token_id: String,
        _purchaser: String,
        _amount: Coin,
    ) -> Result<HookResponse, ContractError> {
        Ok(HookResponse::default())
    }
    /// Called when an `ExecuteMsg::Burn` message is received
    fn on_burn(
        &self,
        _deps: &DepsMut,
        _info: MessageInfo,
        _env: Env,
        _token_id: String,
    ) -> Result<HookResponse, ContractError> {
        Ok(HookResponse::default())
    }
    /// Called when an `ExecuteMsg::Archive` message is received
    fn on_archive(
        &self,
        _deps: &DepsMut,
        _info: MessageInfo,
        _env: Env,
        _token_id: String,
    ) -> Result<HookResponse, ContractError> {
        Ok(HookResponse::default())
    }
}
