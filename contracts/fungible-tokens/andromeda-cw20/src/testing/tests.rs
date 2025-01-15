use crate::contract::{execute, instantiate, query};
use crate::testing::mock_querier::mock_dependencies_custom;
use andromeda_fungible_tokens::cw20::{ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::ado_base::permissioning::{LocalPermission, Permission};
use andromeda_std::ado_base::rates::{LocalRate, LocalRateType, LocalRateValue, PercentRate, Rate};
use andromeda_std::ado_contract::ADOContract;
use andromeda_std::amp::{AndrAddr, Recipient};
use andromeda_std::common::context::ExecuteContext;

use andromeda_std::{error::ContractError, testing::mock_querier::MOCK_KERNEL_CONTRACT};
use andromeda_testing::economics_msg::generate_economics_message;
use cosmwasm_std::{attr, Decimal, Event};
use cosmwasm_std::{
    testing::{mock_env, mock_info},
    to_json_binary, Addr, DepsMut, Response, Uint128,
};

use cw20::{Cw20Coin, Cw20ReceiveMsg};
use cw20_base::state::BALANCES;

use super::mock_querier::MOCK_CW20_CONTRACT;

fn init(deps: DepsMut) -> Response {
    let msg = InstantiateMsg {
        name: MOCK_CW20_CONTRACT.into(),
        symbol: "Symbol".into(),
        decimals: 6,
        initial_balances: vec![
            Cw20Coin {
                amount: 1000u128.into(),
                address: "sender".to_string(),
            },
            Cw20Coin {
                amount: 1u128.into(),
                address: "rates_recipient".to_string(),
            },
            Cw20Coin {
                amount: 1u128.into(),
                address: "royalty_recipient".to_string(),
            },
        ],
        mint: None,
        marketing: None,

        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let info = mock_info("owner", &[]);
    instantiate(deps, mock_env(), info, msg).unwrap()
}

#[test]
fn test_andr_query() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut());

    let msg = QueryMsg::Owner {};
    let res = query(deps.as_ref(), mock_env(), msg);
    // Test that the query is hooked up correctly.
    assert!(res.is_ok())
}

#[test]
fn test_transfer() {
    let mut deps = mock_dependencies_custom(&[]);
    let res = init(deps.as_mut());
    assert_eq!(
        Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("type", "cw20")
            .add_attribute("kernel_address", MOCK_KERNEL_CONTRACT)
            .add_attribute("owner", "owner"),
        res
    );

    assert_eq!(
        Uint128::from(1000u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("sender"))
            .unwrap()
    );

    let msg = ExecuteMsg::Transfer {
        recipient: AndrAddr::from_string("other"),
        amount: 100u128.into(),
    };

    // Set a royalty of 10% to be paid to royalty_recipient
    let rate = Rate::Local(LocalRate {
        rate_type: LocalRateType::Deductive,
        recipient: Recipient {
            address: AndrAddr::from_string("royalty_recipient".to_string()),
            msg: None,
            ibc_recovery_address: None,
        },
        value: LocalRateValue::Percent(PercentRate {
            percent: Decimal::percent(10),
        }),
        description: None,
    });

    // Set rates
    ADOContract::default()
        .set_rates(deps.as_mut().storage, "Transfer", rate)
        .unwrap();

    // The expected events for the royalty
    let expected_event = Event::new("royalty").add_attributes(vec![
        attr("deducted", "10cosmos2contract"),
        attr("payment", "royalty_recipient<10cosmos2contract"),
    ]);

    // Blacklist the sender who otherwise would have been able to call the function successfully
    let permission = Permission::Local(LocalPermission::blacklisted(None, None));
    let actors = vec![AndrAddr::from_string("sender")];
    let action = "Transfer";
    let ctx = ExecuteContext::new(deps.as_mut(), mock_info("owner", &[]), mock_env());
    ADOContract::default()
        .execute_set_permission(ctx, actors, action, permission)
        .unwrap();
    let info = mock_info("sender", &[]);
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();

    assert_eq!(err, ContractError::Unauthorized {});

    // Now whitelist the sender, that should allow him to call the function successfully
    let permission = Permission::Local(LocalPermission::whitelisted(None, None));
    let actors = vec![AndrAddr::from_string("sender")];
    let action = "Transfer";
    let ctx = ExecuteContext::new(deps.as_mut(), mock_info("owner", &[]), mock_env());
    ADOContract::default()
        .execute_set_permission(ctx, actors, action, permission)
        .unwrap();
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_event(expected_event)
            .add_attribute("action", "transfer")
            .add_attribute("from", "sender")
            .add_attribute("to", "other")
            .add_attribute("amount", "90")
            .add_submessage(generate_economics_message("sender", "Transfer")),
        res
    );

    // Funds deducted from the sender (100 for send, 10 for tax).
    assert_eq!(
        Uint128::from(900u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("sender"))
            .unwrap()
    );

    // Funds given to the receiver. Remove 10 for the royalty
    assert_eq!(
        Uint128::from(100u128 - 10u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("other"))
            .unwrap()
    );

    // Royalty given to royalty_recipient
    assert_eq!(
        Uint128::from(1u128 + 10u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("royalty_recipient"))
            .unwrap()
    );
}

#[test]
fn test_send() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("sender", &[]);

    let res = init(deps.as_mut());

    assert_eq!(
        Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("type", "cw20")
            .add_attribute("kernel_address", MOCK_KERNEL_CONTRACT)
            .add_attribute("owner", "owner"),
        res
    );

    assert_eq!(
        Uint128::from(1000u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("sender"))
            .unwrap()
    );

    let rate = Rate::Local(LocalRate {
        rate_type: LocalRateType::Additive,
        recipient: Recipient {
            address: AndrAddr::from_string("rates_recipient".to_string()),
            msg: None,
            ibc_recovery_address: None,
        },
        value: LocalRateValue::Percent(PercentRate {
            percent: Decimal::percent(10),
        }),
        description: None,
    });

    // Set rates
    ADOContract::default()
        .set_rates(deps.as_mut().storage, "Send", rate)
        .unwrap();

    let msg = ExecuteMsg::Send {
        contract: AndrAddr::from_string("contract".to_string()),
        amount: 100u128.into(),
        msg: to_json_binary(&"msg").unwrap(),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let expected_event = Event::new("tax")
        .add_attributes(vec![attr("payment", "rates_recipient<10cosmos2contract")]);

    assert_eq!(
        Response::new()
            .add_attribute("action", "send")
            .add_attribute("from", "sender")
            .add_attribute("to", "contract")
            .add_attribute("amount", "100")
            .add_message(
                Cw20ReceiveMsg {
                    sender: "sender".into(),
                    amount: 100u128.into(),
                    msg: to_json_binary(&"msg").unwrap(),
                }
                .into_cosmos_msg("contract")
                .unwrap(),
            )
            .add_event(expected_event)
            .add_submessage(generate_economics_message("sender", "Send")),
        res
    );

    // Funds deducted from the sender (100 for send, 10 for tax).
    assert_eq!(
        Uint128::from(1_000u128 - 100u128 - 10u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("sender"))
            .unwrap()
    );

    // Funds given to the receiver.
    assert_eq!(
        Uint128::from(100u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("contract"))
            .unwrap()
    );

    // The rates recipient started with a balance of 1, and received 10 from the tax
    assert_eq!(
        Uint128::from(1u128 + 10u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("rates_recipient"))
            .unwrap()
    );
}
