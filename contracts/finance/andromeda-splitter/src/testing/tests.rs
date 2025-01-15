use andromeda_std::{
    amp::{
        messages::{AMPMsg, AMPPkt},
        recipient::Recipient,
    },
    common::{expiration::Expiry, Milliseconds},
    error::ContractError,
};
use cosmwasm_std::{
    attr, from_json,
    testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR},
    to_json_binary, BankMsg, Coin, CosmosMsg, Decimal, DepsMut, Response, SubMsg, Timestamp,
};
pub const OWNER: &str = "creator";

use super::mock_querier::MOCK_KERNEL_CONTRACT;

use crate::{
    contract::{execute, instantiate, query},
    state::SPLITTER,
    testing::mock_querier::mock_dependencies_custom,
};
use andromeda_finance::splitter::{
    AddressPercent, ExecuteMsg, GetSplitterConfigResponse, InstantiateMsg, QueryMsg, Splitter,
};

fn init(deps: DepsMut) -> Response {
    let mock_recipient: Vec<AddressPercent> = vec![AddressPercent {
        recipient: Recipient::from_string(String::from("some_address")),
        percent: Decimal::percent(100),
    }];
    let msg = InstantiateMsg {
        owner: Some(OWNER.to_owned()),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        recipients: mock_recipient,
        lock_time: Some(Expiry::FromNow(Milliseconds(86400000))),
        default_recipient: None,
    };

    let info = mock_info("owner", &[]);
    instantiate(deps, mock_env(), info, msg).unwrap()
}

#[test]
fn test_instantiate() {
    let mut deps = mock_dependencies_custom(&[]);
    let res = init(deps.as_mut());
    assert_eq!(0, res.messages.len());
}

#[test]
fn test_different_lock_times() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    // Current time
    env.block.time = Timestamp::from_seconds(1724920577);
    // Set a lock time that's less than 1 day in milliseconds
    let mut lock_time = Expiry::FromNow(Milliseconds(60_000));

    let msg = InstantiateMsg {
        owner: Some(OWNER.to_owned()),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        recipients: vec![],
        lock_time: Some(lock_time),
        default_recipient: None,
    };

    let info = mock_info(OWNER, &[]);
    let err = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();

    assert_eq!(err, ContractError::LockTimeTooShort {});

    // Set a lock time that's more than 1 year in milliseconds
    lock_time = Expiry::FromNow(Milliseconds(31_708_800_000));

    let msg = InstantiateMsg {
        owner: Some(OWNER.to_owned()),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        recipients: vec![],
        lock_time: Some(lock_time),
        default_recipient: None,
    };

    let err = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();

    assert_eq!(err, ContractError::LockTimeTooLong {});

    // Set a lock time for 20 days in milliseconds
    lock_time = Expiry::FromNow(Milliseconds(1728000000));

    let msg = InstantiateMsg {
        owner: Some(OWNER.to_owned()),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        recipients: vec![AddressPercent {
            recipient: Recipient::from_string(String::from("some_address")),
            percent: Decimal::percent(100),
        }],
        lock_time: Some(lock_time),
        default_recipient: None,
    };

    let info = mock_info(OWNER, &[]);
    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    // Here we begin testing Expiry::AtTime
    // Set a lock time that's less than 1 day from current time
    lock_time = Expiry::AtTime(Milliseconds(1724934977000));

    let msg = InstantiateMsg {
        owner: Some(OWNER.to_owned()),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        recipients: vec![],
        lock_time: Some(lock_time),
        default_recipient: None,
    };

    let info = mock_info(OWNER, &[]);
    let err = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::LockTimeTooShort {});

    // Set a lock time that's more than 1 year from current time in milliseconds
    lock_time = Expiry::AtTime(Milliseconds(1788006977000));

    let msg = InstantiateMsg {
        owner: Some(OWNER.to_owned()),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        recipients: vec![],
        lock_time: Some(lock_time),
        default_recipient: None,
    };

    let info = mock_info(OWNER, &[]);
    let err = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::LockTimeTooLong {});

    // Set a valid lock time
    lock_time = Expiry::AtTime(Milliseconds(1725021377000));

    let msg = InstantiateMsg {
        owner: Some(OWNER.to_owned()),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        recipients: vec![AddressPercent {
            recipient: Recipient::from_string(String::from("some_address")),
            percent: Decimal::percent(100),
        }],
        lock_time: Some(lock_time),
        default_recipient: None,
    };

    let info = mock_info(OWNER, &[]);
    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());
}

#[test]
fn test_execute_update_lock() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res = init(deps.as_mut());

    let env = mock_env();

    let current_time = env.block.time.seconds();
    // 2 days in milliseconds
    let lock_time = 172800000;

    // Start off with an expiration that's behind current time (expired)
    let splitter = Splitter {
        recipients: vec![],
        lock: Milliseconds::from_seconds(current_time - 1),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let msg = ExecuteMsg::UpdateLock {
        lock_time: Expiry::FromNow(Milliseconds(lock_time)),
    };

    let info = mock_info(OWNER, &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    let new_lock = Milliseconds(lock_time)
        .plus_seconds(current_time)
        .plus_milliseconds(Milliseconds(879));
    assert_eq!(
        Response::default().add_attributes(vec![
            attr("action", "update_lock"),
            attr("locked", "1571970219879".to_string())
        ]),
        res
    );

    //check result
    let splitter = SPLITTER.load(deps.as_ref().storage).unwrap();
    assert!(!splitter.lock.is_expired(&env.block));
    assert_eq!(new_lock, splitter.lock);
}

#[test]
fn test_execute_update_recipients() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let _res = init(deps.as_mut());

    let splitter = Splitter {
        recipients: vec![],
        lock: Milliseconds::from_seconds(0),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    // Duplicate recipients
    let duplicate_recipients = vec![
        AddressPercent {
            recipient: Recipient::from_string(String::from("addr1")),
            percent: Decimal::percent(40),
        },
        AddressPercent {
            recipient: Recipient::from_string(String::from("addr1")),
            percent: Decimal::percent(60),
        },
    ];
    let msg = ExecuteMsg::UpdateRecipients {
        recipients: duplicate_recipients,
    };

    let info = mock_info(OWNER, &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(ContractError::DuplicateRecipient {}, res.unwrap_err());

    let recipients = vec![
        AddressPercent {
            recipient: Recipient::from_string(String::from("addr1")),
            percent: Decimal::percent(40),
        },
        AddressPercent {
            recipient: Recipient::from_string(String::from("addr2")),
            percent: Decimal::percent(60),
        },
    ];
    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipients.clone(),
    };

    let info = mock_info("incorrect_owner", &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg.clone());
    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());

    let info = mock_info(OWNER, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        Response::default().add_attributes(vec![attr("action", "update_recipients")]),
        res
    );

    //check result
    let splitter = SPLITTER.load(deps.as_ref().storage).unwrap();
    assert_eq!(splitter.recipients, recipients);
}

#[test]
fn test_execute_send() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let _res: Response = init(deps.as_mut());

    let sender_funds_amount = 10000u128;

    let info = mock_info(OWNER, &[Coin::new(sender_funds_amount, "uluna")]);

    let recip_address1 = "address1".to_string();
    let recip_percent1 = 10; // 10%

    let recip_address2 = "address2".to_string();
    let recip_percent2 = 20; // 20%

    let recip_address3 = "address3".to_string();
    let recip_percent3 = 50; // 50%

    let recip1 = Recipient::from_string(recip_address1);
    let recip2 = Recipient::from_string(recip_address2);
    let recip3 = Recipient::from_string(recip_address3.clone());

    let config_recipient = vec![AddressPercent {
        recipient: recip3.clone(),
        percent: Decimal::percent(recip_percent3),
    }];

    let recipient = vec![
        AddressPercent {
            recipient: recip1.clone(),
            percent: Decimal::percent(recip_percent1),
        },
        AddressPercent {
            recipient: recip2.clone(),
            percent: Decimal::percent(recip_percent2),
        },
    ];
    let msg = ExecuteMsg::Send { config: None };

    let amp_msg_1 = recip1
        .generate_amp_msg(&deps.as_ref(), Some(vec![Coin::new(1000, "uluna")]))
        .unwrap();
    let amp_msg_2 = recip2
        .generate_amp_msg(&deps.as_ref(), Some(vec![Coin::new(2000, "uluna")]))
        .unwrap();
    let amp_pkt = AMPPkt::new(
        MOCK_CONTRACT_ADDR.to_string(),
        MOCK_CONTRACT_ADDR.to_string(),
        vec![amp_msg_1, amp_msg_2],
    );
    let amp_msg = amp_pkt
        .to_sub_msg(
            MOCK_KERNEL_CONTRACT,
            Some(vec![Coin::new(1000, "uluna"), Coin::new(2000, "uluna")]),
            1,
        )
        .unwrap();

    let splitter = Splitter {
        recipients: recipient.clone(),
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let expected_res = Response::new()
        .add_submessages(vec![
            SubMsg::new(
                // refunds remainder to sender
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: OWNER.to_string(),
                    amount: vec![Coin::new(7000, "uluna")], // 10000 * 0.7   remainder
                }),
            ),
            amp_msg,
        ])
        .add_attributes(vec![attr("action", "send"), attr("sender", "creator")]);

    assert_eq!(res, expected_res);

    // Test send with config
    let msg = ExecuteMsg::Send {
        config: Some(config_recipient),
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    let amp_msg_1 = recip3
        .generate_amp_msg(&deps.as_ref(), Some(vec![Coin::new(5000, "uluna")]))
        .unwrap();

    let amp_pkt = AMPPkt::new(
        MOCK_CONTRACT_ADDR.to_string(),
        MOCK_CONTRACT_ADDR.to_string(),
        vec![amp_msg_1],
    );
    let amp_msg = amp_pkt
        .to_sub_msg(
            MOCK_KERNEL_CONTRACT,
            Some(vec![Coin::new(5000, "uluna")]),
            1,
        )
        .unwrap();
    let expected_res = Response::new()
        .add_submessages(vec![
            SubMsg::new(
                // refunds remainder to sender
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: OWNER.to_string(),
                    amount: vec![Coin::new(5000, "uluna")], // 10000 * 0.5   remainder
                }),
            ),
            amp_msg.clone(),
        ])
        .add_attributes(vec![attr("action", "send"), attr("sender", "creator")]);

    assert_eq!(res, expected_res);

    // Test send with default recipient
    let msg = ExecuteMsg::Send { config: None };
    SPLITTER
        .save(
            deps.as_mut().storage,
            &Splitter {
                recipients: recipient,
                lock: Milliseconds::default(),
                default_recipient: Some(recip3.clone()),
            },
        )
        .unwrap();

    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    let amp_msg_1 = recip1
        .generate_amp_msg(&deps.as_ref(), Some(vec![Coin::new(1000, "uluna")]))
        .unwrap();
    let amp_msg_2 = recip2
        .generate_amp_msg(&deps.as_ref(), Some(vec![Coin::new(2000, "uluna")]))
        .unwrap();
    let amp_pkt = AMPPkt::new(
        MOCK_CONTRACT_ADDR.to_string(),
        MOCK_CONTRACT_ADDR.to_string(),
        vec![amp_msg_1, amp_msg_2],
    );
    let amp_msg = amp_pkt
        .to_sub_msg(
            MOCK_KERNEL_CONTRACT,
            Some(vec![Coin::new(1000, "uluna"), Coin::new(2000, "uluna")]),
            1,
        )
        .unwrap();

    let expected_res = Response::new()
        .add_submessages(vec![
            SubMsg::new(
                // refunds remainder to sender
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: recip_address3,
                    amount: vec![Coin::new(7000, "uluna")], // 10000 * 0.7   remainder
                }),
            ),
            amp_msg,
        ])
        .add_attributes(vec![attr("action", "send"), attr("sender", "creator")]);

    assert_eq!(res, expected_res);
}

#[test]
fn test_execute_send_ado_recipient() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let _res: Response = init(deps.as_mut());

    let sender_funds_amount = 10000u128;
    let info = mock_info(OWNER, &[Coin::new(sender_funds_amount, "uluna")]);

    let recip_address1 = "address1".to_string();
    let recip_percent1 = 10; // 10%

    let recip_address2 = "address2".to_string();
    let recip_percent2 = 20; // 20%

    let recip1 = Recipient::from_string(recip_address1);
    let recip2 = Recipient::from_string(recip_address2);

    let recipient = vec![
        AddressPercent {
            recipient: recip1.clone(),
            percent: Decimal::percent(recip_percent1),
        },
        AddressPercent {
            recipient: recip2.clone(),
            percent: Decimal::percent(recip_percent2),
        },
    ];
    let msg = ExecuteMsg::Send { config: None };

    let amp_msg_1 = recip1
        .generate_amp_msg(&deps.as_ref(), Some(vec![Coin::new(1000, "uluna")]))
        .unwrap();
    let amp_msg_2 = recip2
        .generate_amp_msg(&deps.as_ref(), Some(vec![Coin::new(2000, "uluna")]))
        .unwrap();
    let amp_pkt = AMPPkt::new(
        MOCK_CONTRACT_ADDR.to_string(),
        MOCK_CONTRACT_ADDR.to_string(),
        vec![amp_msg_1, amp_msg_2],
    );
    let amp_msg = amp_pkt
        .to_sub_msg(
            MOCK_KERNEL_CONTRACT,
            Some(vec![Coin::new(1000, "uluna"), Coin::new(2000, "uluna")]),
            1,
        )
        .unwrap();

    let splitter = Splitter {
        recipients: recipient,
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let res = execute(deps.as_mut(), env, info.clone(), msg).unwrap();

    let expected_res = Response::new()
        .add_submessages(vec![
            SubMsg::new(
                // refunds remainder to sender
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: vec![Coin::new(7000, "uluna")], // 10000 * 0.7   remainder
                }),
            ),
            amp_msg,
        ])
        .add_attribute("action", "send")
        .add_attribute("sender", "creator");

    assert_eq!(res, expected_res);
}

#[test]
fn test_handle_packet_exit_with_error_true() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let _res: Response = init(deps.as_mut());

    let sender_funds_amount = 0u128;
    let info = mock_info(OWNER, &[Coin::new(sender_funds_amount, "uluna")]);

    let recip_address1 = "address1".to_string();
    let recip_percent1 = 10; // 10%

    let recip_percent2 = 20; // 20%

    let recipient = vec![
        AddressPercent {
            recipient: Recipient::from_string(recip_address1.clone()),
            percent: Decimal::percent(recip_percent1),
        },
        AddressPercent {
            recipient: Recipient::from_string(recip_address1.clone()),
            percent: Decimal::percent(recip_percent2),
        },
    ];
    let pkt = AMPPkt::new(
        info.clone().sender,
        "cosmos2contract",
        vec![AMPMsg::new(
            recip_address1,
            to_json_binary(&ExecuteMsg::Send { config: None }).unwrap(),
            Some(vec![Coin::new(0, "uluna")]),
        )],
    );
    let msg = ExecuteMsg::AMPReceive(pkt);

    let splitter = Splitter {
        recipients: recipient,
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();

    assert_eq!(
        err,
        ContractError::InvalidFunds {
            msg: "Amount must be non-zero".to_string(),
        }
    );
}

#[test]
fn test_query_splitter() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let splitter = Splitter {
        recipients: vec![],
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let query_msg = QueryMsg::GetSplitterConfig {};
    let res = query(deps.as_ref(), env, query_msg).unwrap();
    let val: GetSplitterConfigResponse = from_json(res).unwrap();

    assert_eq!(val.config, splitter);
}

#[test]
fn test_execute_send_error() {
    //Executes send with more than 5 tokens [ACK-04]
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let _res: Response = init(deps.as_mut());

    let sender_funds_amount = 10000u128;
    let owner = "creator";
    let info = mock_info(
        owner,
        &vec![
            Coin::new(sender_funds_amount, "uluna"),
            Coin::new(sender_funds_amount, "uluna"),
            Coin::new(sender_funds_amount, "uluna"),
            Coin::new(sender_funds_amount, "uluna"),
            Coin::new(sender_funds_amount, "uluna"),
            Coin::new(sender_funds_amount, "uluna"),
        ],
    );

    let recip_address1 = "address1".to_string();
    let recip_percent1 = 10; // 10%

    let recip_address2 = "address2".to_string();
    let recip_percent2 = 20; // 20%

    let recipient = vec![
        AddressPercent {
            recipient: Recipient::from_string(recip_address1),
            percent: Decimal::percent(recip_percent1),
        },
        AddressPercent {
            recipient: Recipient::from_string(recip_address2),
            percent: Decimal::percent(recip_percent2),
        },
    ];
    let msg = ExecuteMsg::Send { config: None };

    let splitter = Splitter {
        recipients: recipient,
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();

    let expected_res = ContractError::ExceedsMaxAllowedCoins {};

    assert_eq!(res, expected_res);
}

#[test]
fn test_update_app_contract() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res: Response = init(deps.as_mut());

    let info = mock_info(OWNER, &[]);

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

#[test]
fn test_update_app_contract_invalid_recipient() {
    let mut deps = mock_dependencies_custom(&[]);
    let _res: Response = init(deps.as_mut());

    let info = mock_info(OWNER, &[]);

    let msg = ExecuteMsg::UpdateAppContract {
        address: "z".to_string(),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    // assert_eq!(
    //     ContractError::InvalidComponent {
    //         name: "z".to_string()
    //     },
    //     res.unwrap_err()
    // );
    assert!(res.is_err())
}
