use crate::ado_base::rates::{AllRatesResponse, Rate, RatesResponse};
use crate::amp::Recipient;
use crate::common::{context::ExecuteContext, Funds};
use crate::error::ContractError;
use crate::os::aos_querier::AOSQuerier;
use cosmwasm_std::Uint128;
use cosmwasm_std::{coin as create_coin, ensure, Coin, Deps, Response, Storage};
use cw20::Cw20Coin;
use cw_storage_plus::Map;

use super::ADOContract;

pub fn rates<'a>() -> Map<'a, &'a str, Vec<Rate>> {
    Map::new("rates")
}

impl<'a> ADOContract<'a> {
    /// Sets rates
    pub fn set_rates(
        &self,
        store: &mut dyn Storage,
        action: impl Into<String>,
        rates: Vec<Rate>,
    ) -> Result<(), ContractError> {
        let action: String = action.into();
        self.rates.save(store, &action, &rates)?;
        Ok(())
    }

    pub fn execute_set_rates(
        &self,
        ctx: ExecuteContext,
        action: impl Into<String>,
        mut rates: Vec<Rate>,
    ) -> Result<Response, ContractError> {
        // Ensure the sender is the contract owner
        ensure!(
            Self::is_contract_owner(self, ctx.deps.storage, ctx.info.sender.as_str())?,
            ContractError::Unauthorized {}
        );

        let action = action.into();

        // Iterate over the rates, validating and updating as needed
        for rate in &mut rates {
            // Validate rates
            rate.validate_rate(ctx.deps.as_ref())?;

            // Update local rates if recipients are empty
            if let Rate::Local(ref mut local_rate) = rate {
                if local_rate.recipients.is_empty() {
                    local_rate
                        .recipients
                        .push(Recipient::new(ctx.info.sender.clone(), None));
                }
            }
        }

        // Save the updated rates
        self.set_rates(ctx.deps.storage, action, rates)?;

        Ok(Response::default().add_attributes(vec![("action", "set_rates")]))
    }

    pub fn remove_rates(
        &self,
        store: &mut dyn Storage,
        action: impl Into<String>,
    ) -> Result<(), ContractError> {
        let action: String = action.into();
        self.rates.remove(store, &action);
        Ok(())
    }
    pub fn execute_remove_rates(
        &self,
        ctx: ExecuteContext,
        action: impl Into<String>,
    ) -> Result<Response, ContractError> {
        ensure!(
            Self::is_contract_owner(self, ctx.deps.storage, ctx.info.sender.as_str())?,
            ContractError::Unauthorized {}
        );
        let action: String = action.into();
        self.remove_rates(ctx.deps.storage, action.clone())?;

        Ok(Response::default().add_attributes(vec![
            ("action", "remove_rates"),
            ("removed_action", &action),
        ]))
    }

    pub fn get_rates(
        &self,
        deps: Deps,
        action: impl Into<String>,
    ) -> Result<Option<Vec<Rate>>, ContractError> {
        let action: String = action.into();
        Ok(rates().may_load(deps.storage, &action)?)
    }

    pub fn get_all_rates(&self, deps: Deps) -> Result<AllRatesResponse, ContractError> {
        // Initialize a vector to hold all rates
        let mut all_rates: Vec<(String, Vec<Rate>)> = Vec::new();

        // Iterate over all keys and load the corresponding rate
        rates()
            .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
            .for_each(|item| {
                if let Ok((action, rate)) = item {
                    all_rates.push((action, rate));
                }
            });

        Ok(AllRatesResponse { all_rates })
    }

    pub fn query_deducted_funds(
        self,
        deps: Deps,
        action: impl Into<String>,
        funds: Funds,
    ) -> Result<Option<RatesResponse>, ContractError> {
        let action: String = action.into();
        let rate = self.rates.may_load(deps.storage, &action)?;
        match rate {
            Some(rates) => {
                let (coin, is_native): (Coin, bool) = match funds {
                    Funds::Native(coin) => {
                        ensure!(
                            !coin.amount.is_zero(),
                            ContractError::InvalidFunds {
                                msg: "Zero amounts are prohibited".to_string()
                            }
                        );
                        (coin, true)
                    }
                    Funds::Cw20(cw20_coin) => {
                        ensure!(
                            !cw20_coin.amount.is_zero(),
                            ContractError::InvalidFunds {
                                msg: "Zero amounts are prohibited".to_string()
                            }
                        );
                        (
                            create_coin(cw20_coin.amount.u128(), cw20_coin.address),
                            false,
                        )
                    }
                };
                let mut all_msgs = vec![];
                let mut all_events = vec![];
                let mut all_leftover_funds = vec![];
                for rate in rates {
                    let (mut msgs, mut events, mut leftover_funds) = match rate {
                        Rate::Local(local_rate) => {
                            local_rate.generate_response(deps, coin.clone(), is_native)?
                        }
                        Rate::Contract(rates_address) => {
                            // Query rates contract
                            let addr = rates_address.get_raw_address(&deps)?;
                            let rate = AOSQuerier::get_rate(&deps.querier, &addr, &action)?;
                            rate.generate_response(deps, coin.clone(), is_native)?
                        }
                    };
                    all_msgs.append(&mut msgs);
                    all_events.append(&mut events);
                    all_leftover_funds.append(&mut leftover_funds);
                }
                let total_dedcuted_funds: Uint128 = all_leftover_funds
                    .iter()
                    .map(|x| coin.amount - x.amount)
                    .sum();
                let total_funds = coin.amount.checked_sub(total_dedcuted_funds)?;
                Ok(Some(RatesResponse {
                    msgs: all_msgs,
                    leftover_funds: if is_native {
                        Funds::Native(Coin {
                            denom: coin.denom,
                            amount: total_funds,
                        })
                    } else {
                        Funds::Cw20(Cw20Coin {
                            amount: total_funds,
                            address: coin.denom,
                        })
                    },
                    events: all_events,
                }))
            }
            None => Ok(None),
        }
    }
}
#[cfg(test)]
#[cfg(feature = "rates")]

mod tests {

    use cosmwasm_std::{
        coin,
        testing::{mock_dependencies, mock_env},
        Addr,
    };

    use crate::{
        ado_base::rates::{LocalRate, LocalRateType, LocalRateValue},
        amp::{AndrAddr, Recipient},
    };

    use super::*;
    #[test]
    fn test_rates_crud() {
        let mut deps = mock_dependencies();
        let _env = mock_env();
        let contract = ADOContract::default();
        contract
            .owner
            .save(deps.as_mut().storage, &Addr::unchecked("owner"))
            .unwrap();

        let expected_rate = vec![Rate::Local(LocalRate {
            rate_type: LocalRateType::Additive,
            recipients: vec![Recipient {
                address: AndrAddr::from_string("owner".to_string()),
                msg: None,
                ibc_recovery_address: None,
            }],
            value: LocalRateValue::Flat(coin(100_u128, "uandr")),
            description: None,
        })];

        let action = "deposit";
        // set rates
        ADOContract::set_rates(&contract, &mut deps.storage, action, expected_rate.clone())
            .unwrap();

        let rate = ADOContract::default()
            .rates
            .load(&deps.storage, action)
            .unwrap();

        assert_eq!(rate, expected_rate);

        // get rates
        let rate = ADOContract::default()
            .get_rates(deps.as_ref(), action)
            .unwrap();
        assert_eq!(expected_rate, rate.unwrap());

        // remove rates
        ADOContract::remove_rates(&contract, &mut deps.storage, action).unwrap();
        let rate = ADOContract::default()
            .get_rates(deps.as_ref(), action)
            .unwrap();
        assert!(rate.is_none());
    }
}