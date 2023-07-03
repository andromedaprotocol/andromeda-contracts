use crate::contract::{execute, instantiate, query};
use crate::testing::mock_querier::{mock_dependencies_custom, MOCK_RATES_CONTRACT};
use andromeda_fungible_tokens::cw20::{ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::testing::mock_querier::MOCK_ADDRESS_LIST_CONTRACT;
use andromeda_std::{
    ado_base::Module, amp::addresses::AndrAddr, error::ContractError,
    testing::mock_querier::MOCK_KERNEL_CONTRACT,
};
use cosmwasm_std::{
    testing::{mock_env, mock_info},
    to_binary, Addr, DepsMut, Event, Response, StdError, Uint128,
};
use cw20::{Cw20Coin, Cw20ReceiveMsg};
use cw20_base::state::BALANCES;

use super::mock_querier::MOCK_CW20_CONTRACT;

fn init(deps: DepsMut, modules: Option<Vec<Module>>) -> Response {
    let msg = InstantiateMsg {
        name: MOCK_CW20_CONTRACT.into(),
        symbol: "Symbol".into(),
        decimals: 6,
        initial_balances: vec![Cw20Coin {
            amount: 1000u128.into(),
            address: "sender".to_string(),
        }],
        mint: None,
        marketing: None,
        modules,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let info = mock_info("owner", &[]);
    instantiate(deps, mock_env(), info, msg).unwrap()
}

#[test]
fn test_andr_query() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut(), None);

    let msg = QueryMsg::Owner {};
    let res = query(deps.as_ref(), mock_env(), msg);
    // Test that the query is hooked up correctly.
    assert!(res.is_ok())
}

//TODO: FIX THIS TEST, RATES ARE NOT WORKING
#[test]
fn test_transfer() {
    let modules: Vec<Module> = vec![
        Module {
            name: Some(MOCK_RATES_CONTRACT.to_owned()),
            address: AndrAddr::from_string(MOCK_RATES_CONTRACT.to_owned()),

            is_mutable: false,
        },
        Module {
            name: Some(MOCK_ADDRESS_LIST_CONTRACT.to_owned()),
            address: AndrAddr::from_string(MOCK_ADDRESS_LIST_CONTRACT.to_owned()),

            is_mutable: false,
        },
    ];

    let mut deps = mock_dependencies_custom(&[]);
    let res = init(deps.as_mut(), Some(modules));
    assert_eq!(
        Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("type", "cw20")
            .add_attribute("action", "register_module")
            .add_attribute("module_idx", "1")
            .add_attribute("action", "register_module")
            .add_attribute("module_idx", "2")
            .add_attribute("action", "register_module")
            .add_attribute("module_idx", "3")
            .add_attribute("method", "instantiate"),
        res
    );

    assert_eq!(
        Uint128::from(1000u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("sender"))
            .unwrap()
    );

    let msg = ExecuteMsg::Transfer {
        recipient: "other".into(),
        amount: 100u128.into(),
    };

    let not_whitelisted_info = mock_info("not_whitelisted", &[]);
    let res = execute(deps.as_mut(), mock_env(), not_whitelisted_info, msg.clone());
    assert_eq!(
        ContractError::Std(StdError::generic_err(
            "Querier contract error: InvalidAddress"
        )),
        res.unwrap_err()
    );
    let info = mock_info("sender", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            // .add_event(Event::new("Royalty"))
            // .add_event(Event::new("Tax"))
            .add_attribute("action", "transfer")
            .add_attribute("from", "sender")
            .add_attribute("to", "other")
            //TODO: RATES NOT WORKING
            .add_attribute("amount", "100"),
        res
    );

    // TODO: RATES NOT WORKING
    // Funds deducted from the sender (100 for send, 10 for tax).
    assert_eq!(
        Uint128::from(900u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("sender"))
            .unwrap()
    );

    // Funds given to the receiver.
    assert_eq!(
        Uint128::from(100u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("other"))
            .unwrap()
    );

    // Royalty given to rates_recipient
    // assert_eq!(
    //     Uint128::from(0u128),
    //     BALANCES
    //         .load(deps.as_ref().storage, &Addr::unchecked("rates_recipient"))
    //         .unwrap()
    // );
}

#[test]
fn test_send() {
    let modules: Vec<Module> = vec![
        Module {
            name: Some(MOCK_RATES_CONTRACT.to_owned()),
            address: AndrAddr::from_string(MOCK_RATES_CONTRACT.to_owned()),

            is_mutable: false,
        },
        //TODO uncomment once address_list is updated

        // Module {
        //     name: Some(MOCK_ADDRESSLIST_CONTRACT.to_owned()),
        //     address: AndrAddr::from_string(MOCK_ADDRESSLIST_CONTRACT.to_owned()),

        //     is_mutable: false,
        // },
    ];

    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("sender", &[]);

    let res = init(deps.as_mut(), Some(modules));

    assert_eq!(
        Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("type", "cw20")
            .add_attribute("action", "register_module")
            .add_attribute("module_idx", "1"), // .add_attribute("action", "register_module")
        // .add_attribute("module_idx", "2")
        // .add_attribute("action", "register_module")
        // .add_attribute("module_idx", "3")
        res
    );

    assert_eq!(
        Uint128::from(1000u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("sender"))
            .unwrap()
    );

    let msg = ExecuteMsg::Send {
        contract: "contract".into(),
        amount: 100u128.into(),
        msg: to_binary(&"msg").unwrap(),
    };

    let not_whitelisted_info = mock_info("not_whitelisted", &[]);
    let res = execute(deps.as_mut(), mock_env(), not_whitelisted_info, msg.clone());
    assert_eq!(
        ContractError::Std(StdError::generic_err(
            "Querier contract error: InvalidAddress"
        )),
        res.unwrap_err()
    );

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_event(Event::new("Royalty"))
            .add_event(Event::new("Tax"))
            .add_attribute("action", "send")
            .add_attribute("from", "sender")
            .add_attribute("to", "contract")
            .add_attribute("amount", "90")
            .add_message(
                Cw20ReceiveMsg {
                    sender: "sender".into(),
                    amount: 90u128.into(),
                    msg: to_binary(&"msg").unwrap(),
                }
                .into_cosmos_msg("contract")
                .unwrap(),
            ),
        res
    );

    // Funds deducted from the sender (100 for send, 10 for tax).
    assert_eq!(
        Uint128::from(890u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("sender"))
            .unwrap()
    );

    // Funds given to the receiver.
    assert_eq!(
        Uint128::from(90u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("contract"))
            .unwrap()
    );

    // Royalty given to rates_recipient (10 from royalty and 10 from tax)
    assert_eq!(
        Uint128::from(20u128),
        BALANCES
            .load(deps.as_ref().storage, &Addr::unchecked("rates_recipient"))
            .unwrap()
    );
}

#[test]
fn test_update_app_contract() {
    let mut deps = mock_dependencies_custom(&[]);

    let modules: Vec<Module> = vec![Module {
        name: Some(MOCK_RATES_CONTRACT.to_owned()),
        address: AndrAddr::from_string(MOCK_RATES_CONTRACT.to_owned()),
        is_mutable: false,
    }];

    let info = mock_info("app_contract", &[]);
    let _res = init(deps.as_mut(), Some(modules));

    let msg = ExecuteMsg::UpdateAppContract {
        address: "app_contract".to_string(),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        Response::new()
            .add_attribute("action", "update_app_contract")
            .add_attribute("address", "app_contract"),
        res
    );
}
