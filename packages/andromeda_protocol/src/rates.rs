use common::{
    ado_base::{
        hooks::{AndromedaHook, OnFundsTransferResponse},
        query_get,
        recipient::Recipient,
        AndromedaMsg, AndromedaQuery,
    },
    encode_binary,
    error::ContractError,
    primitive::{GetValueResponse, Primitive},
    require, Funds,
};
use cosmwasm_std::{
    BankMsg, Coin, CosmosMsg, Decimal, Fraction, QuerierWrapper, QueryRequest, SubMsg, Uint128,
    WasmQuery,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub rates: Vec<RateInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AndrReceive(AndromedaMsg),
    UpdateRates { rates: Vec<RateInfo> },
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    AndrQuery(AndromedaQuery),
    AndrHook(AndromedaHook),
    Payments {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PaymentsResponse {
    pub payments: Vec<RateInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RateInfo {
    pub rate: Rate,
    pub is_additive: bool,
    pub description: Option<String>,
    pub receivers: Vec<Recipient>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ADORate {
    /// The address of the primitive contract.
    pub address: String,
    /// The key of the primitive in the primitive contract.
    pub key: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// An enum used to define various types of fees
pub enum Rate {
    /// A flat rate fee
    Flat(Coin),
    /// A percentage fee
    Percent(PercentRate),
    External(ADORate),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// This is added such that both Rate::Flat and Rate::Percent have the same level of nesting which
// makes it easier to work with on the frontend.
pub struct PercentRate {
    pub percent: Decimal,
}

impl From<Decimal> for Rate {
    fn from(decimal: Decimal) -> Self {
        Rate::Percent(PercentRate { percent: decimal })
    }
}

impl Rate {
    /// Validates that a given rate is non-zero. It is expected that the Rate is not an
    /// External Rate.
    pub fn is_non_zero(&self) -> Result<bool, ContractError> {
        match self {
            Rate::Flat(coin) => Ok(!coin.amount.is_zero()),
            Rate::Percent(PercentRate { percent }) => Ok(!percent.is_zero()),
            Rate::External(_) => Err(ContractError::UnexpectedExternalRate {}),
        }
    }

    /// Validates `self` and returns an "unwrapped" version of itself wherein if it is an External
    /// Rate, the actual rate value is retrieved from the Primitive Contract.
    pub fn validate(&self, querier: &QuerierWrapper) -> Result<Rate, ContractError> {
        let rate = self.clone().get_rate(querier)?;
        require(rate.is_non_zero()?, ContractError::InvalidRate {})?;

        if let Rate::Percent(PercentRate { percent }) = rate {
            require(percent <= Decimal::one(), ContractError::InvalidRate {})?;
        }

        Ok(rate)
    }

    /// If `self` is Flat or Percent it returns itself. Otherwise it queries the primitive contract
    /// and retrieves the actual Flat or Percent rate.
    fn get_rate(self, querier: &QuerierWrapper) -> Result<Rate, ContractError> {
        match self {
            Rate::Flat(_) => Ok(self),
            Rate::Percent(_) => Ok(self),
            Rate::External(ado_rate) => {
                let response: GetValueResponse = query_get(
                    Some(encode_binary(&ado_rate.key)?),
                    ado_rate.address,
                    querier,
                )?;
                match response.value {
                    Primitive::Coin(coin) => Ok(Rate::Flat(coin)),
                    Primitive::Decimal(value) => Ok(Rate::from(value)),
                    _ => Err(ContractError::ParsingError {
                        err: "Stored rate is not a coin or Decimal".to_string(),
                    }),
                }
            }
        }
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

/// Gets the amount of tax paid by iterating over the `msgs` and comparing it to the
/// difference between the base amount and the amount left over after royalties.
/// It is assumed that each bank message has a single Coin to send as transfer
/// agreements only accept a single Coin. It is also assumed that the result will always be
/// non-negative.
///
/// # Arguments
///
/// * `msgs` - The vector of submessages containing fund transfers
/// * `base_amount` - The amount paid before tax.
/// * `remaining_amount_after_royalties` - The amount remaining of the base_amount after royalties
///                                        are applied
/// Returns the amount of tax necessary to be paid on top of the `base_amount`.
pub fn get_tax_amount(
    msgs: &[SubMsg],
    base_amount: Uint128,
    remaining_amount_after_royalties: Uint128,
) -> Uint128 {
    let deducted_amount = base_amount - remaining_amount_after_royalties;
    msgs.iter()
        .map(|msg| {
            if let CosmosMsg::Bank(BankMsg::Send { amount, .. }) = &msg.msg {
                amount[0].amount
            } else {
                Uint128::zero()
            }
        })
        .reduce(|total, amount| total + amount)
        .unwrap_or_else(Uint128::zero)
        - deducted_amount
}

pub fn on_required_payments(
    querier: QuerierWrapper,
    addr: String,
    amount: Funds,
) -> Result<OnFundsTransferResponse, ContractError> {
    let res: OnFundsTransferResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: addr,
        msg: encode_binary(&QueryMsg::AndrQuery(AndromedaQuery::Get(Some(
            encode_binary(&amount)?,
        ))))?,
    }))?;

    Ok(res)
}

/// Calculates a fee amount given a `Rate` and payment amount.
///
/// ## Arguments
/// * `fee_rate` - The `Rate` of the fee to be paid
/// * `payment` - The amount used to calculate the fee
///
/// Returns the fee amount in a `Coin` struct.
pub fn calculate_fee(fee_rate: Rate, payment: &Coin) -> Result<Coin, ContractError> {
    match fee_rate {
        Rate::Flat(rate) => Ok(Coin::new(rate.amount.u128(), rate.denom)),
        Rate::Percent(PercentRate { percent }) => {
            // [COM-03] Make sure that fee_rate between 0 and 100.
            require(
                // No need for rate >=0 due to type limits (Question: Should add or remove?)
                percent <= Decimal::one() && !percent.is_zero(),
                ContractError::InvalidRate {},
            )
            .unwrap();
            let mut fee_amount = payment.amount * percent;

            // Always round any remainder up and prioritise the fee receiver.
            // Inverse of percent will always exist.
            let reversed_fee = fee_amount * percent.inv().unwrap();
            if payment.amount > reversed_fee {
                // [COM-1] Added checked add to fee_amount rather than direct increment
                fee_amount = fee_amount.checked_add(1u128.into())?;
            }
            Ok(Coin::new(fee_amount.u128(), payment.denom.clone()))
        }
        Rate::External(_) => Err(ContractError::UnexpectedExternalRate {}),
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::mock_querier::{mock_dependencies_custom, MOCK_PRIMITIVE_CONTRACT};
    use cosmwasm_std::coin;

    use super::*;

    #[test]
    fn test_validate_external_rate() {
        let mut deps = mock_dependencies_custom(&[]);

        let rate = Rate::External(ADORate {
            address: MOCK_PRIMITIVE_CONTRACT.to_string(),
            key: Some("percent".to_string()),
        });
        let validated_rate = rate.validate(&deps.as_mut().querier).unwrap();
        let expected_rate = Rate::from(Decimal::percent(1));
        assert_eq!(expected_rate, validated_rate);

        let rate = Rate::External(ADORate {
            address: MOCK_PRIMITIVE_CONTRACT.to_string(),
            key: Some("flat".to_string()),
        });
        let validated_rate = rate.validate(&deps.as_mut().querier).unwrap();
        let expected_rate = Rate::Flat(coin(1u128, "uusd"));
        assert_eq!(expected_rate, validated_rate);
    }

    #[test]
    fn test_calculate_fee() {
        let payment = coin(101, "uluna");
        let expected = Ok(coin(5, "uluna"));
        let fee = Rate::from(Decimal::percent(4));

        let received = calculate_fee(fee, &payment);

        assert_eq!(expected, received);

        assert_eq!(expected, received);

        let payment = coin(125, "uluna");
        let fee = Rate::Flat(Coin {
            amount: Uint128::from(5_u128),
            denom: "uluna".to_string(),
        });

        let received = calculate_fee(fee, &payment);

        assert_eq!(expected, received);
    }
}
