use cosmwasm_std::{Coin, DepsMut, Env, Event, MessageInfo, StdError, StdResult};

use crate::{
    modules::common::{add_payment, calculate_fee, require},
    modules::hooks::{MessageHooks, PaymentAttribute},
    modules::Rate,
    modules::{Module, ModuleDefinition},
};

use super::hooks::{HookResponse, ATTR_DESC, ATTR_PAYMENT};

pub const TAX_EVENT_ID: &str = "tax";

pub struct Taxable {
    pub rate: Rate,
    pub receivers: Vec<String>,
    pub description: Option<String>,
}

impl Module for Taxable {
    fn validate(&self, _modules: Vec<crate::modules::ModuleDefinition>) -> StdResult<bool> {
        require(
            self.receivers.len() > 0,
            StdError::generic_err("Cannot apply a tax with no receiving addresses"),
        )?;
        // require(self.rate > 0, StdError::generic_err("Tax must be non-zero"))?;
        match self.rate.clone() {
            Rate::Flat(rate) => {
                require(
                    rate.amount.u128() > 0,
                    StdError::generic_err("Tax must be non-zero"),
                )?;
            }
            Rate::Percent(rate) => {
                require(rate > 0, StdError::generic_err("Tax must be non-zero"))?;
            }
        }

        if self.description.clone().is_some() {
            require(
                self.description.clone().unwrap().len() <= 200,
                StdError::generic_err("Module description can be at most 200 characters long"),
            )?;
        }

        Ok(true)
    }
    fn as_definition(&self) -> ModuleDefinition {
        ModuleDefinition::Taxable {
            rate: self.rate.clone(),
            receivers: self.receivers.clone(),
            description: None,
        }
    }
}

impl MessageHooks for Taxable {
    fn on_agreed_transfer(
        &self,
        _deps: &DepsMut,
        _info: MessageInfo,
        env: Env,
        payments: &mut Vec<cosmwasm_std::BankMsg>,
        _owner: String,
        _purchaser: String,
        agreed_payment: Coin,
    ) -> StdResult<HookResponse> {
        let _contract_addr = env.contract.address;
        let tax_amount = calculate_fee(self.rate.clone(), agreed_payment);

        let mut resp = HookResponse::default();
        let mut event = Event::new(TAX_EVENT_ID);

        match self.description.clone() {
            Some(desc) => {
                event = event.add_attribute(ATTR_DESC, desc);
            }
            None => {}
        }

        for receiver in self.receivers.to_vec() {
            add_payment(payments, receiver.clone(), tax_amount.clone());
            event = event.add_attribute(
                ATTR_PAYMENT,
                PaymentAttribute {
                    receiver: receiver.clone(),
                    amount: tax_amount.clone(),
                }
                .to_string(),
            );
        }
        resp = resp.add_event(event);

        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{
        coin, coins,
        testing::{mock_dependencies, mock_env, mock_info},
        BankMsg,
    };

    use super::*;

    #[test]
    fn test_taxable_validate() {
        let t = Taxable {
            rate: Rate::Percent(2),
            receivers: vec![String::default()],
            description: None,
        };

        assert_eq!(t.validate(vec![]).unwrap(), true);

        let t_invalidtax = Taxable {
            rate: Rate::Percent(0),
            receivers: vec![String::default()],
            description: None,
        };

        assert_eq!(
            t_invalidtax.validate(vec![]).unwrap_err(),
            StdError::generic_err("Tax must be non-zero")
        );

        let t_invalidrecv = Taxable {
            rate: Rate::Percent(2),
            receivers: vec![],
            description: None,
        };

        assert_eq!(
            t_invalidrecv.validate(vec![]).unwrap_err(),
            StdError::generic_err("Cannot apply a tax with no receiving addresses")
        );
    }

    #[test]

    fn test_taxable_on_agreed_transfer() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("sender", &[]);
        let env = mock_env();
        let receivers = vec![String::from("recv1"), String::from("recv2")];
        let t = Taxable {
            rate: Rate::Percent(3),
            receivers: receivers.clone(),
            description: None,
        };

        let agreed_transfer_amount = coin(117, "uluna");
        let tax_amount = 4;
        let owner = String::from("owner");
        let purchaser = String::from("purchaser");
        let mut payments = vec![];

        t.on_agreed_transfer(
            &deps.as_mut(),
            info.clone(),
            env.clone(),
            &mut payments,
            owner.clone(),
            purchaser.clone(),
            agreed_transfer_amount.clone(),
        )
        .unwrap();

        assert_eq!(payments.len(), 2);

        let first_payment = BankMsg::Send {
            to_address: String::from("recv1"),
            amount: coins(tax_amount, &agreed_transfer_amount.denom.to_string()),
        };
        let second_payment = BankMsg::Send {
            to_address: String::from("recv2"),
            amount: coins(tax_amount, &agreed_transfer_amount.denom.to_string()),
        };

        assert_eq!(payments[0], first_payment);
        assert_eq!(payments[1], second_payment);
    }

    #[test]

    fn test_taxable_on_agreed_transfer_resp() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("sender", &[]);
        let env = mock_env();
        let desc = "Some tax module";
        let receivers = vec![String::from("recv1"), String::from("recv2")];
        let t = Taxable {
            rate: Rate::Percent(1),
            receivers: receivers.clone(),
            description: Some(desc.to_string()),
        };

        let agreed_transfer_amount = coin(100, "uluna");
        let owner = String::from("owner");
        let purchaser = String::from("purchaser");
        let mut payments = vec![];

        let resp = t
            .on_agreed_transfer(
                &deps.as_mut(),
                info.clone(),
                env.clone(),
                &mut payments,
                owner.clone(),
                purchaser.clone(),
                agreed_transfer_amount.clone(),
            )
            .unwrap();

        assert_eq!(resp.events.len(), 1);
        assert_eq!(resp.events[0].ty, "tax");
        assert_eq!(resp.events[0].attributes.len(), 3);
        assert_eq!(resp.events[0].attributes[0].key, ATTR_DESC);
        assert_eq!(resp.events[0].attributes[0].value, desc.to_string());
        assert_eq!(resp.events[0].attributes[1].key, ATTR_PAYMENT);
        assert_eq!(
            resp.events[0].attributes[1].value,
            PaymentAttribute {
                receiver: t.receivers[0].clone(),
                amount: calculate_fee(t.rate, agreed_transfer_amount)
            }
            .to_string()
        );
    }
}
