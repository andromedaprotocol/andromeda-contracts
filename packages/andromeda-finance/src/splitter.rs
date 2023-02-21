use andromeda_os::kernel::ExecuteMsg as KernelExecuteMsg;
use andromeda_os::messages::{AMPMsg, AMPPkt, ExecuteMsg as AMPExecuteMsg, ReplyGasExit};
use andromeda_os::recipient::AMPRecipient;
use common::{
    ado_base::{modules::Module, AndromedaMsg, AndromedaQuery},
    encode_binary,
    error::ContractError,
};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{ensure, BankMsg, Binary, Coin, CosmosMsg, Decimal, SubMsg, WasmMsg};
use cw_utils::Expiration;

#[cw_serde]
pub struct AddressPercent {
    pub recipient: AMPRecipient,
    pub percent: Decimal,
}

#[cw_serde]
/// A config struct for a `Splitter` contract.
pub struct Splitter {
    /// The vector of recipients for the contract. Anytime a `Send` execute message is sent the amount sent will be divided amongst these recipients depending on their assigned percentage.
    pub recipients: Vec<AddressPercent>,
    /// Whether or not the contract is currently locked. This restricts updating any config related fields.
    pub lock: Expiration,
}

#[cw_serde]
pub struct InstantiateMsg {
    /// The vector of recipients for the contract. Anytime a `Send` execute message is
    /// sent the amount sent will be divided amongst these recipients depending on their assigned percentage.
    pub recipients: Vec<AddressPercent>,
    pub lock_time: Option<u64>,
    pub modules: Option<Vec<Module>>,
    pub kernel_address: Option<String>,
}

impl InstantiateMsg {
    pub fn validate(&self) -> Result<bool, ContractError> {
        validate_recipient_list(self.recipients.clone())?;
        Ok(true)
    }
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Update the recipients list. Only executable by the contract owner when the contract is not locked.
    UpdateRecipients {
        recipients: Vec<AddressPercent>,
    },
    /// Used to lock/unlock the contract allowing the config to be updated.
    UpdateLock {
        lock_time: u64,
    },
    /// Divides any attached funds to the message amongst the recipients list.
    Send {
        reply_gas: ReplyGasExit,
        packet: Option<AMPPkt>,
    },

    AndrReceive(AndromedaMsg),
    AMPReceive(AMPPkt),
}

#[cw_serde]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(AndromedaQuery)]
    AndrQuery(AndromedaQuery),
    /// The current config of the Splitter contract
    #[returns(GetSplitterConfigResponse)]
    GetSplitterConfig {},
}

#[cw_serde]
pub struct GetSplitterConfigResponse {
    pub config: Splitter,
}

/// Ensures that a given list of recipients for a `splitter` contract is valid:
///
/// * Must include at least one recipient
/// * The combined percentage of the recipients must not exceed 100
pub fn validate_recipient_list(recipients: Vec<AddressPercent>) -> Result<bool, ContractError> {
    ensure!(
        !recipients.is_empty(),
        ContractError::EmptyRecipientsList {}
    );

    let mut percent_sum: Decimal = Decimal::zero();
    for rec in recipients {
        // += operation is not supported for decimal.
        percent_sum += rec.percent;
    }

    ensure!(
        percent_sum <= Decimal::one(),
        ContractError::AmountExceededHundredPrecent {}
    );

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_recipient_list() {
        let empty_recipients = vec![];
        let res = validate_recipient_list(empty_recipients).unwrap_err();
        assert_eq!(res, ContractError::EmptyRecipientsList {});

        let inadequate_recipients = vec![AddressPercent {
            recipient: AMPRecipient::from_string(String::from("Some Address")),
            percent: Decimal::percent(150),
        }];
        let res = validate_recipient_list(inadequate_recipients).unwrap_err();
        assert_eq!(res, ContractError::AmountExceededHundredPrecent {});

        let valid_recipients = vec![
            AddressPercent {
                recipient: AMPRecipient::from_string(String::from("Some Address")),
                percent: Decimal::percent(50),
            },
            AddressPercent {
                recipient: AMPRecipient::from_string(String::from("Some Address")),
                percent: Decimal::percent(50),
            },
        ];

        let res = validate_recipient_list(valid_recipients).unwrap();
        assert!(res);
    }
}
