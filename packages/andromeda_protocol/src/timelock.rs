use cosmwasm_std::{Addr, Api, BlockInfo, Coin};
use cw721::Expiration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::communication::{AndromedaMsg, AndromedaQuery};
use crate::error::ContractError;
use crate::{modules::address_list::AddressListModule, require};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
/// Struct used to define funds being held in Escrow
pub struct Escrow {
    /// Funds being held within the Escrow
    pub coins: Vec<Coin>,
    /// Optional expiration for the Escrow
    pub expiration: Option<Expiration>,
    /// The recipient of the funds once Expiration is reached
    pub recipient: String,
    /// The owner of the Escrow.
    pub owner: Addr,
}

impl Escrow {
    /// Used to check the validity of an Escrow before it is stored.
    ///
    /// * Escrowed funds cannot be empty
    /// * The Escrow recipient must be a valid address
    /// * Expiration cannot be "Never" or before current time/block
    pub fn validate(&self, api: &dyn Api, block: &BlockInfo) -> Result<bool, ContractError> {
        require(!self.coins.is_empty(), ContractError::EmptyFunds {})?;
        require(
            api.addr_validate(&self.recipient).is_ok(),
            ContractError::InvalidAddress {},
        )?;

        if let Some(expiration) = self.expiration {
            match expiration {
                //ACK-01 Change (Check before deleting comment)
                Expiration::AtTime(time) => {
                    if time < block.time {
                        return Err(ContractError::ExpirationInPast {});
                    }
                }
                Expiration::Never {} => {
                    return Err(ContractError::ExpirationNotSpecified {});
                }
                _ => {}
            }
        }

        Ok(true)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// An optional address list module to restrict usage of the contract
    pub address_list: Option<AddressListModule>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AndrReceive(AndromedaMsg),
    /// Hold funds in Escrow
    HoldFunds {
        expiration: Option<Expiration>,
        recipient: Option<String>,
    },
    /// Update the optional address list module
    UpdateAddressList {
        address_list: Option<AddressListModule>,
    },
    /// Release funds held in Escrow
    ReleaseFunds {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    AndrQuery(AndromedaQuery),
    /// Queries funds held by an address
    GetLockedFunds {
        address: String,
    },
    /// The current config of the contract
    GetTimelockConfig {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct GetLockedFundsResponse {
    pub funds: Option<Escrow>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct GetTimelockConfigResponse {
    pub address_list: Option<AddressListModule>,
    pub address_list_contract: Option<String>,
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::mock_dependencies;
    use cosmwasm_std::{coin, Timestamp};

    use super::*;

    #[test]
    fn test_validate() {
        let deps = mock_dependencies(&[]);
        let expiration = Expiration::AtHeight(1);
        let coins = vec![coin(100u128, "uluna")];
        let recipient = String::from("owner");
        let owner = Addr::unchecked("owner");

        let valid_escrow = Escrow {
            recipient: recipient.clone(),
            coins: coins.clone(),
            expiration: Some(expiration),
            owner: owner.clone(),
        };
        let block = BlockInfo {
            height: 1000,
            time: Timestamp::from_seconds(4444),
            chain_id: "foo".to_string(),
        };
        let resp = valid_escrow.validate(deps.as_ref().api, &block).unwrap();
        assert!(resp);

        let valid_escrow = Escrow {
            recipient: recipient.clone(),
            coins: coins.clone(),
            expiration: None,
            owner: owner.clone(),
        };
        let block = BlockInfo {
            height: 1000,
            time: Timestamp::from_seconds(3333),
            chain_id: "foo".to_string(),
        };
        let resp = valid_escrow.validate(deps.as_ref().api, &block).unwrap();
        assert!(resp);

        let invalid_recipient_escrow = Escrow {
            recipient: String::default(),
            coins: coins.clone(),
            expiration: Some(expiration),
            owner: owner.clone(),
        };

        let resp = invalid_recipient_escrow
            .validate(deps.as_ref().api, &block)
            .unwrap_err();
        assert_eq!(ContractError::InvalidAddress {}, resp);

        let invalid_coins_escrow = Escrow {
            recipient: recipient.clone(),
            coins: vec![],
            expiration: Some(expiration),
            owner: owner.clone(),
        };

        let resp = invalid_coins_escrow
            .validate(deps.as_ref().api, &block)
            .unwrap_err();
        assert_eq!(ContractError::EmptyFunds {}, resp);

        let invalid_expiration_escrow = Escrow {
            recipient,
            coins,
            expiration: Some(Expiration::Never {}),
            owner: owner.clone(),
        };

        let resp = invalid_expiration_escrow
            .validate(deps.as_ref().api, &block)
            .unwrap_err();
        assert_eq!(ContractError::ExpirationNotSpecified {}, resp);
    }
}
