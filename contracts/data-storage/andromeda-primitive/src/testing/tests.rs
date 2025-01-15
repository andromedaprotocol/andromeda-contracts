use crate::contract::{execute, query};
use andromeda_data_storage::primitive::{
    ExecuteMsg, GetValueResponse, Primitive, PrimitiveRestriction, QueryMsg,
};
use cosmwasm_std::{
    coin, from_json, testing::mock_env, BankMsg, Binary, CosmosMsg, Decimal, Response, SubMsg,
};

use andromeda_std::{
    ado_base::rates::{LocalRate, LocalRateType, LocalRateValue, PercentRate, Rate, RatesMessage},
    ado_contract::ADOContract,
    amp::{AndrAddr, Recipient},
    error::ContractError,
    testing::mock_querier::{mock_dependencies_custom, MOCK_CW20_CONTRACT},
};

use super::mock::{
    delete_value, proper_initialization, query_value, set_value, set_value_with_funds,
};

#[test]
fn test_instantiation() {
    proper_initialization(PrimitiveRestriction::Private);
}

#[test]
fn test_set_and_update_value_with_key() {
    let (mut deps, info) = proper_initialization(PrimitiveRestriction::Private);
    let key = String::from("key");
    let value = Primitive::String("value".to_string());
    set_value(
        deps.as_mut(),
        &Some(key.clone()),
        &value,
        info.sender.as_ref(),
    )
    .unwrap();

    let query_res: GetValueResponse = query_value(deps.as_ref(), &Some(key.clone())).unwrap();

    assert_eq!(
        GetValueResponse {
            key: key.clone(),
            value
        },
        query_res
    );

    let value = Primitive::String("value2".to_string());
    set_value(
        deps.as_mut(),
        &Some(key.clone()),
        &value,
        info.sender.as_ref(),
    )
    .unwrap();

    let query_res: GetValueResponse = query_value(deps.as_ref(), &Some(key.clone())).unwrap();

    assert_eq!(GetValueResponse { key, value }, query_res);
}

#[test]
fn test_set_value_with_tax() {
    let (mut deps, info) = proper_initialization(PrimitiveRestriction::Private);
    let key = String::from("key");
    let value = Primitive::String("value".to_string());
    let tax_recipient = "tax_recipient";

    // Set percent rates
    let set_percent_rate_msg = ExecuteMsg::Rates(RatesMessage::SetRate {
        action: "PrimitiveSetValue".to_string(),
        rate: Rate::Local(LocalRate {
            rate_type: LocalRateType::Additive,
            recipient: Recipient {
                address: AndrAddr::from_string(String::default()),
                msg: None,
                ibc_recovery_address: None,
            },
            value: LocalRateValue::Percent(PercentRate {
                percent: Decimal::one(),
            }),
            description: None,
        }),
    });

    let err = execute(
        deps.as_mut(),
        mock_env(),
        info.clone(),
        set_percent_rate_msg,
    )
    .unwrap_err();

    assert_eq!(err, ContractError::InvalidRate {});

    // Make sure sender is set as recipient when the recipients vector is empty
    let rate: Rate = Rate::Local(LocalRate {
        rate_type: LocalRateType::Additive,
        recipient: Recipient {
            address: AndrAddr::from_string("creator".to_string()),
            msg: None,
            ibc_recovery_address: None,
        },
        value: LocalRateValue::Flat(coin(20_u128, MOCK_CW20_CONTRACT)),
        description: None,
    });

    let msg = ExecuteMsg::Rates(RatesMessage::SetRate {
        action: "SetValue".to_string(),
        rate,
    });
    execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let queried_rates = ADOContract::default()
        .get_rates(deps.as_ref(), "SetValue".to_string())
        .unwrap();
    assert_eq!(
        queried_rates.unwrap(),
        Rate::Local(LocalRate {
            rate_type: LocalRateType::Additive,
            recipient: Recipient::new(AndrAddr::from_string("creator"), None),
            value: LocalRateValue::Flat(coin(20_u128, MOCK_CW20_CONTRACT)),
            description: None,
        })
    );

    let rate: Rate = Rate::Local(LocalRate {
        rate_type: LocalRateType::Additive,
        recipient: Recipient {
            address: AndrAddr::from_string(tax_recipient.to_string()),
            msg: None,
            ibc_recovery_address: None,
        },
        value: LocalRateValue::Flat(coin(20_u128, "uandr")),
        description: None,
    });

    // Set rates
    ADOContract::default()
        .set_rates(deps.as_mut().storage, "SetValue", rate)
        .unwrap();

    // Sent the exact amount required for tax
    let res = set_value_with_funds(
        deps.as_mut(),
        &Some(key.clone()),
        &value,
        info.sender.as_ref(),
        coin(20_u128, "uandr".to_string()),
    )
    .unwrap();
    let expected_response: Response = Response::new()
        .add_submessage(SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: tax_recipient.to_string(),
            amount: vec![coin(20, "uandr")],
        })))
        .add_attributes(vec![
            ("method", "set_value"),
            ("sender", "creator"),
            ("key", "key"),
        ])
        .add_attribute("value", format!("{value:?}"));
    assert_eq!(expected_response, res);

    // Sent less than amount required for tax
    let err = set_value_with_funds(
        deps.as_mut(),
        &Some(key.clone()),
        &value,
        info.sender.as_ref(),
        coin(19_u128, "uandr".to_string()),
    )
    .unwrap_err();
    assert_eq!(err, ContractError::InsufficientFunds {});

    // Sent more than required amount for tax
    let res = set_value_with_funds(
        deps.as_mut(),
        &Some(key.clone()),
        &value,
        info.sender.as_ref(),
        coin(200_u128, "uandr".to_string()),
    )
    .unwrap();
    let expected_response: Response = Response::new()
        .add_submessage(SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: tax_recipient.to_string(),
            amount: vec![coin(20, "uandr")],
        })))
        // 200 was sent, but the tax is only 20, so we send back the difference
        .add_submessage(SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: "creator".to_string(),
            amount: vec![coin(180, "uandr")],
        })))
        .add_attributes(vec![
            ("method", "set_value"),
            ("sender", "creator"),
            ("key", "key"),
        ])
        .add_attribute("value", format!("{value:?}"));
    assert_eq!(expected_response, res);
}

#[test]
fn test_set_and_update_value_without_key() {
    let (mut deps, info) = proper_initialization(PrimitiveRestriction::Private);
    let key = None;
    let value = Primitive::String("value".to_string());
    set_value(deps.as_mut(), &key, &value, info.sender.as_ref()).unwrap();

    let query_res: GetValueResponse = query_value(deps.as_ref(), &key).unwrap();

    assert_eq!(
        GetValueResponse {
            key: key.clone().unwrap_or("default".into()),
            value
        },
        query_res
    );

    let value = Primitive::String("value2".to_string());
    set_value(deps.as_mut(), &key, &value, info.sender.as_ref()).unwrap();

    let query_res: GetValueResponse = query_value(deps.as_ref(), &key).unwrap();

    assert_eq!(
        GetValueResponse {
            key: "default".to_string(),
            value
        },
        query_res
    );
}

struct TestHandlePrimitive {
    name: &'static str,
    primitive: Primitive,
    expected_error: Option<ContractError>,
}

#[test]
fn test_set_value_invalid() {
    let test_cases = vec![
        TestHandlePrimitive {
            name: "Empty String",
            primitive: Primitive::String("".to_string()),
            expected_error: Some(ContractError::EmptyString {}),
        },
        TestHandlePrimitive {
            name: "Empty coin denom",
            primitive: Primitive::Coin(coin(1_u128, "".to_string())),
            expected_error: Some(ContractError::InvalidDenom { msg: None }),
        },
        TestHandlePrimitive {
            name: "Empty Binary",
            primitive: Primitive::Binary(Binary::default()),
            expected_error: Some(ContractError::EmptyString {}),
        },
    ];

    for test in test_cases {
        let deps = mock_dependencies_custom(&[]);

        let res = test.primitive.validate(&deps.api);

        if let Some(err) = test.expected_error {
            assert_eq!(res.unwrap_err(), err, "{}", test.name);
            continue;
        }

        assert!(res.is_ok())
    }
}

#[test]
fn test_delete_value() {
    let (mut deps, info) = proper_initialization(PrimitiveRestriction::Private);
    // Without Key
    let value = Primitive::String("value".to_string());
    set_value(deps.as_mut(), &None, &value, info.sender.as_ref()).unwrap();
    delete_value(deps.as_mut(), &None, info.sender.as_ref()).unwrap();
    query_value(deps.as_ref(), &None).unwrap_err();

    // With key
    let key = Some("key".to_string());
    set_value(deps.as_mut(), &key, &value, info.sender.as_ref()).unwrap();
    delete_value(deps.as_mut(), &key, info.sender.as_ref()).unwrap();
    query_value(deps.as_ref(), &key).unwrap_err();
}

#[test]
fn test_restriction_private() {
    let (mut deps, info) = proper_initialization(PrimitiveRestriction::Private);

    let key = Some("key".to_string());
    let value = Primitive::String("value".to_string());
    let external_user = "external".to_string();

    // Set Value as owner
    set_value(deps.as_mut(), &key, &value, info.sender.as_ref()).unwrap();
    delete_value(deps.as_mut(), &key, info.sender.as_ref()).unwrap();
    // This should error, key is deleted
    query_value(deps.as_ref(), &key).unwrap_err();

    // Set Value as external user
    // This should error
    set_value(deps.as_mut(), &key, &value, &external_user).unwrap_err();
    // Set a value by owner so we can test delete for it
    set_value(deps.as_mut(), &key, &value, info.sender.as_ref()).unwrap();
    // Delete value set by owner by external user
    // This will error
    delete_value(deps.as_mut(), &key, &external_user).unwrap_err();

    // Key is still present
    query_value(deps.as_ref(), &key).unwrap();
}

#[test]
fn test_restriction_public() {
    let (mut deps, info) = proper_initialization(PrimitiveRestriction::Public);

    let key = Some("key".to_string());
    let value = Primitive::String("value".to_string());
    let external_user = "external".to_string();

    // Set Value as owner
    set_value(deps.as_mut(), &key, &value, info.sender.as_ref()).unwrap();
    delete_value(deps.as_mut(), &key, info.sender.as_ref()).unwrap();
    // This should error, key is deleted
    query_value(deps.as_ref(), &key).unwrap_err();

    // Set Value as external user
    set_value(deps.as_mut(), &key, &value, &external_user).unwrap();
    delete_value(deps.as_mut(), &key, &external_user).unwrap();
    // This should error, key is deleted
    query_value(deps.as_ref(), &key).unwrap_err();

    // Set Value as owner
    set_value(deps.as_mut(), &key, &value, info.sender.as_ref()).unwrap();
    // Delete the value as external user
    delete_value(deps.as_mut(), &key, &external_user).unwrap();
    // This should error, key is deleted
    query_value(deps.as_ref(), &key).unwrap_err();
}

#[test]
fn test_restriction_restricted() {
    let (mut deps, info) = proper_initialization(PrimitiveRestriction::Restricted);

    let key = Some("key".to_string());
    let value = Primitive::String("value".to_string());
    let value2 = Primitive::String("value2".to_string());
    let external_user = "external".to_string();
    let external_user2 = "external2".to_string();

    // Set Value as owner
    set_value(deps.as_mut(), &key, &value, info.sender.as_ref()).unwrap();
    delete_value(deps.as_mut(), &key, info.sender.as_ref()).unwrap();
    // This should error, key is deleted
    query_value(deps.as_ref(), &key).unwrap_err();

    // Set Value as external user
    set_value(deps.as_mut(), &key, &value, &external_user).unwrap();
    delete_value(deps.as_mut(), &key, &external_user).unwrap();
    // This should error, key is deleted
    query_value(deps.as_ref(), &key).unwrap_err();

    // Set Value as owner and try to delete as external user
    set_value(deps.as_mut(), &key, &value, info.sender.as_ref()).unwrap();
    // Try to modify it as external user
    set_value(deps.as_mut(), &key, &value2, &external_user).unwrap_err();
    // Delete the value as external user, this should error
    delete_value(deps.as_mut(), &key, &external_user).unwrap_err();
    // Key is still present
    query_value(deps.as_ref(), &key).unwrap();

    let key = Some("key2".to_string());
    // Set Value as external user and try to delete as owner
    set_value(deps.as_mut(), &key, &value, &external_user).unwrap();
    // Delete the value as external user, this will success as owner has permission to do anything
    delete_value(deps.as_mut(), &key, info.sender.as_ref()).unwrap();
    // Key is not present, this will error
    query_value(deps.as_ref(), &key).unwrap_err();

    let key = Some("key3".to_string());
    // Set Value as external user 1 and try to delete as external user 2
    set_value(deps.as_mut(), &key, &value, &external_user).unwrap();
    // Delete the value as external user, this will error
    delete_value(deps.as_mut(), &key, &external_user2).unwrap_err();
    // Key is present
    query_value(deps.as_ref(), &key).unwrap();
}

#[test]
fn test_query_all_key() {
    let (mut deps, info) = proper_initialization(PrimitiveRestriction::Private);

    let keys: Vec<String> = vec!["key1".into(), "key2".into()];
    let value = Primitive::String("value".to_string());
    for key in keys.clone() {
        set_value(deps.as_mut(), &Some(key), &value, info.sender.as_ref()).unwrap();
    }

    let res: Vec<String> =
        from_json(query(deps.as_ref(), mock_env(), QueryMsg::AllKeys {}).unwrap()).unwrap();

    assert_eq!(res, keys)
}

#[test]
fn test_query_owner_keys() {
    let (mut deps, _) = proper_initialization(PrimitiveRestriction::Restricted);

    let keys: Vec<String> = vec!["1".into(), "2".into()];
    let value = Primitive::String("value".to_string());
    let sender = "sender1".to_string();
    for key in keys.clone() {
        set_value(
            deps.as_mut(),
            &Some(format!("{sender}-{key}")),
            &value,
            &sender,
        )
        .unwrap();
    }

    let sender = "sender2".to_string();
    for key in keys {
        set_value(
            deps.as_mut(),
            &Some(format!("{sender}-{key}")),
            &value,
            &sender,
        )
        .unwrap();
    }

    let res: Vec<String> =
        from_json(query(deps.as_ref(), mock_env(), QueryMsg::AllKeys {}).unwrap()).unwrap();
    assert!(res.len() == 4, "Not all keys added");

    let res: Vec<String> = from_json(
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::OwnerKeys {
                owner: AndrAddr::from_string("sender1"),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert!(res.len() == 2, "assertion failed {res:?}", res = res);

    let res: Vec<String> = from_json(
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::OwnerKeys {
                owner: AndrAddr::from_string("sender2"),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert!(res.len() == 2, "assertion failed {res:?}", res = res);
}
