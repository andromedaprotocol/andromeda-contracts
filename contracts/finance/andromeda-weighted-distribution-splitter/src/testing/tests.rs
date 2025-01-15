use andromeda_finance::weighted_splitter::{AddressWeight, ExecuteMsg, InstantiateMsg, Splitter};
use andromeda_std::{
    ado_base::InstantiateMsg as BaseInstantiateMsg,
    ado_contract::ADOContract,
    amp::{recipient::Recipient, AndrAddr},
    common::{expiration::Expiry, Milliseconds},
    error::ContractError,
    testing::mock_querier::{mock_dependencies_custom, MOCK_KERNEL_CONTRACT},
};
use cosmwasm_std::{
    attr,
    testing::{mock_dependencies, mock_env, mock_info},
    BankMsg, Coin, CosmosMsg, DepsMut, QuerierWrapper, Response, SubMsg, Uint128,
};

use crate::{
    contract::{execute, instantiate},
    state::SPLITTER,
};
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MOCK_RECIPIENT1: &str = "recipient1";
const MOCK_RECIPIENT2: &str = "recipient2";
pub const OWNER: &str = "creator";

fn init(deps: DepsMut) -> Response {
    let mock_recipient: Vec<AddressWeight> = vec![AddressWeight {
        recipient: Recipient::from_string(String::from("some_address")),
        weight: Uint128::new(100),
    }];
    let msg = InstantiateMsg {
        owner: Some(OWNER.to_owned()),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        recipients: mock_recipient,
        lock_time: Some(Expiry::FromNow(Milliseconds(86400000))),
        default_recipient: None,
    };

    let info = mock_info(OWNER, &[]);
    instantiate(deps, mock_env(), info, msg).unwrap()
}
#[test]
fn test_update_app_contract() {
    let mut deps = mock_dependencies_custom(&[]);

    let info = mock_info("owner", &[]);
    let msg = InstantiateMsg {
        recipients: vec![
            AddressWeight {
                recipient: Recipient::new(MOCK_RECIPIENT1, None),
                weight: Uint128::new(50),
            },
            AddressWeight {
                recipient: Recipient::new(MOCK_RECIPIENT2, None),
                weight: Uint128::new(50),
            },
        ],
        lock_time: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
        default_recipient: None,
    };

    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

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

// #[test]
// fn test_update_app_contract_invalid_recipient() {
//     let mut deps = mock_dependencies_custom(&[]);

//     let modules: Vec<Module> = vec![Module {
//         name: Some("ks".to_string()),
//         address: AndrAddr::from_string("z".to_string()),
//         is_mutable: false,
//     }];

//     let info = mock_info("app_contract", &[]);
//     let msg = InstantiateMsg {
//
//         recipients: vec![AddressWeight {
//             recipient: Recipient::new(MOCK_RECIPIENT1, None),
//             weight: Uint128::new(100),
//         }],
//         lock_time: Some(100_000),
//         kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
//         owner: None,
//     };

//     let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

//     let msg = ExecuteMsg::UpdateAppContract {
//         address: "app_contract".to_string(),
//     };

//     let res = execute(deps.as_mut(), mock_env(), info, msg);

//     assert_eq!(ContractError::InvalidAddress {}, res.unwrap_err());
// }

#[test]
fn test_instantiate() {
    let mut deps: cosmwasm_std::OwnedDeps<
        cosmwasm_std::MemoryStorage,
        cosmwasm_std::testing::MockApi,
        cosmwasm_std::testing::MockQuerier,
    > = mock_dependencies();
    let env = mock_env();
    let info = mock_info("creator", &[]);
    let msg = InstantiateMsg {
        recipients: vec![AddressWeight {
            recipient: Recipient::from_string(MOCK_RECIPIENT1.to_string()),
            weight: Uint128::new(1),
        }],

        lock_time: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
        default_recipient: None,
    };
    let res = instantiate(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(0, res.messages.len());
}

#[test]
fn test_execute_update_lock() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let current_time = env.block.time.seconds();
    // 2 days in milliseconds
    let lock_time = Milliseconds(172800000);

    let owner = "creator";

    // Start off with an expiration that's behind current time (expired)
    let splitter = Splitter {
        recipients: vec![],
        lock: Milliseconds(current_time - 1),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let msg = ExecuteMsg::UpdateLock {
        lock_time: Expiry::FromNow(lock_time),
    };
    ADOContract::default()
        .instantiate(
            &mut deps.storage,
            mock_env(),
            &deps.api,
            &QuerierWrapper::new(&deps.querier),
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info(owner, &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let new_lock = lock_time
        .plus_seconds(current_time)
        .plus_milliseconds(Milliseconds(879));
    assert_eq!(
        Response::default().add_attributes(vec![
            attr("action", "update_lock"),
            attr("locked", new_lock.to_string())
        ]),
        res
    );

    //check result
    let splitter = SPLITTER.load(deps.as_ref().storage).unwrap();
    assert!(!splitter.lock.is_expired(&env.block));
}

#[test]
fn test_execute_update_lock_too_short() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let current_time = env.block.time.seconds();
    let lock_time = Milliseconds(1);

    let owner = "creator";

    // Start off with an expiration that's behind current time (expired)
    let splitter = Splitter {
        recipients: vec![],
        lock: Milliseconds(current_time - 1),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let msg = ExecuteMsg::UpdateLock {
        lock_time: Expiry::FromNow(lock_time),
    };
    ADOContract::default()
        .instantiate(
            &mut deps.storage,
            mock_env(),
            &deps.api,
            &QuerierWrapper::new(&deps.querier),
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info(owner, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(ContractError::LockTimeTooShort {}, res);
}

#[test]
fn test_execute_update_lock_too_long() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let current_time = env.block.time.seconds();
    // 25 months
    let lock_time = Milliseconds(65_743_650_000);

    let owner = "creator";

    // Start off with an expiration that's behind current time (expired)
    let splitter = Splitter {
        recipients: vec![],
        lock: Milliseconds(current_time - 1),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let msg = ExecuteMsg::UpdateLock {
        lock_time: Expiry::FromNow(lock_time),
    };
    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info(owner, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(ContractError::LockTimeTooLong {}, res);
}

#[test]
fn test_execute_update_lock_already_locked() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let current_time = env.block.time.seconds();

    let lock_time = Milliseconds(172800000);

    let owner = "creator";

    // Start off with an expiration that's ahead current time (unexpired)
    let splitter = Splitter {
        recipients: vec![],
        lock: Milliseconds::default().plus_seconds(current_time + 10_000),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let msg = ExecuteMsg::UpdateLock {
        lock_time: Expiry::FromNow(lock_time),
    };
    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info(owner, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(ContractError::ContractLocked {}, res);
}

#[test]
fn test_execute_update_lock_unauthorized() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let current_time = env.block.time.seconds();
    let lock_time = Milliseconds(100_000);

    let owner = "creator";
    let new_lock = Milliseconds(current_time - 1);

    let splitter = Splitter {
        recipients: vec![],
        lock: new_lock,
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let msg = ExecuteMsg::UpdateLock {
        lock_time: Expiry::FromNow(lock_time),
    };
    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info("incorrect_owner", &[]);
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
}

#[test]
fn test_execute_remove_recipient() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(60),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr3")),
            weight: Uint128::new(50),
        },
    ];

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info(owner, &[]);

    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };
    let splitter = Splitter {
        recipients: recipient,
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::RemoveRecipient {
        recipient: AndrAddr::from_string("addr1"),
    };
    // Try removing a user that isn't in the list
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    let splitter = SPLITTER.load(deps.as_ref().storage).unwrap();
    let expected_splitter = Splitter {
        recipients: vec![
            AddressWeight {
                recipient: Recipient::from_string(String::from("addr3")),
                weight: Uint128::new(50),
            },
            AddressWeight {
                recipient: Recipient::from_string(String::from("addr2")),
                weight: Uint128::new(60),
            },
        ],
        lock: Milliseconds::default(),
        default_recipient: None,
    };
    assert_eq!(expected_splitter, splitter);
    assert_eq!(
        Response::default().add_attributes(vec![attr("action", "removed_recipient")]),
        res
    );

    // check result
    let splitter = SPLITTER.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        splitter.recipients[0],
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr3")),
            weight: Uint128::new(50),
        }
    );
    assert_eq!(splitter.recipients.len(), 2);
}

#[test]
fn test_execute_remove_recipient_not_on_list() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(60),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr3")),
            weight: Uint128::new(50),
        },
    ];

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info(owner, &[]);

    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };
    let splitter = Splitter {
        recipients: recipient,
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // Try removing a user that isn't in the list
    let msg = ExecuteMsg::RemoveRecipient {
        recipient: AndrAddr::from_string("addr10"),
    };

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::UserNotFound {});
}

#[test]
fn test_execute_remove_recipient_contract_locked() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(60),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr3")),
            weight: Uint128::new(50),
        },
    ];

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info(owner, &[]);

    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };
    let splitter = Splitter {
        recipients: recipient.clone(),
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let current_time = env.block.time.seconds();
    let splitter = Splitter {
        recipients: recipient,
        lock: Milliseconds::default().plus_seconds(current_time + 10_000),
        default_recipient: None,
    };
    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let msg = ExecuteMsg::RemoveRecipient {
        recipient: AndrAddr::from_string("addr1"),
    };

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::ContractLocked {});
}

#[test]
fn test_execute_remove_recipient_unauthorized() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(60),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr3")),
            weight: Uint128::new(50),
        },
    ];
    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient,
    };

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info("incorrect_owner", &[]);
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
}

#[test]
fn test_update_recipient_weight() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(60),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr3")),
            weight: Uint128::new(50),
        },
    ];
    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info("incorrect_owner", &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());

    let info = mock_info(owner, &[]);

    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };
    let splitter = Splitter {
        recipients: recipient.clone(),
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // Works
    let splitter = Splitter {
        recipients: recipient,
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();
    let msg = ExecuteMsg::UpdateRecipientWeight {
        recipient: AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(100),
        },
    };
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        Response::default().add_attributes(vec![attr("action", "updated_recipient_weight")]),
        res
    );
    let splitter = SPLITTER.load(deps.as_ref().storage).unwrap();
    let expected_splitter = Splitter {
        recipients: vec![
            AddressWeight {
                recipient: Recipient::from_string(String::from("addr1")),
                weight: Uint128::new(100),
            },
            AddressWeight {
                recipient: Recipient::from_string(String::from("addr2")),
                weight: Uint128::new(60),
            },
            AddressWeight {
                recipient: Recipient::from_string(String::from("addr3")),
                weight: Uint128::new(50),
            },
        ],
        lock: Milliseconds::default(),
        default_recipient: None,
    };
    assert_eq!(expected_splitter, splitter);
}

#[test]
fn test_update_recipient_weight_locked_contract() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(60),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr3")),
            weight: Uint128::new(50),
        },
    ];

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info(owner, &[]);

    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };
    let current_time = env.block.time.seconds();
    let splitter = Splitter {
        recipients: recipient.clone(),
        lock: Milliseconds(current_time - 1),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // Locked contract
    let splitter = Splitter {
        recipients: recipient,
        lock: Milliseconds::default().plus_seconds(current_time + 1),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();
    let msg = ExecuteMsg::UpdateRecipientWeight {
        recipient: AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(100),
        },
    };
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::ContractLocked {});
}

#[test]
fn test_update_recipient_weight_user_not_found() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(60),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr3")),
            weight: Uint128::new(50),
        },
    ];
    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info("incorrect_owner", &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());

    let info = mock_info(owner, &[]);

    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };
    let splitter = Splitter {
        recipients: recipient,
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // User not found
    let msg = ExecuteMsg::UpdateRecipientWeight {
        recipient: AddressWeight {
            recipient: Recipient::from_string(String::from("addr4")),
            weight: Uint128::new(100),
        },
    };
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::UserNotFound {});
}

#[test]

fn test_update_recipient_weight_invalid_weight() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(60),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr3")),
            weight: Uint128::new(50),
        },
    ];
    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info("incorrect_owner", &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());

    let info = mock_info(owner, &[]);

    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };
    let splitter = Splitter {
        recipients: recipient,
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::UpdateRecipientWeight {
        recipient: AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::zero(),
        },
    };
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidWeight {});
}

#[test]
fn test_execute_add_recipient() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(60),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr3")),
            weight: Uint128::new(50),
        },
    ];

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info(owner, &[]);

    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };
    let splitter = Splitter {
        recipients: recipient,
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // Works

    let msg = ExecuteMsg::AddRecipient {
        recipient: AddressWeight {
            recipient: Recipient::from_string(String::from("addr4")),
            weight: Uint128::new(100),
        },
    };

    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        Response::default().add_attributes(vec![attr("action", "added_recipient")]),
        res
    );

    let splitter = SPLITTER.load(deps.as_ref().storage).unwrap();
    let expected_splitter = Splitter {
        recipients: vec![
            AddressWeight {
                recipient: Recipient::from_string(String::from("addr1")),
                weight: Uint128::new(40),
            },
            AddressWeight {
                recipient: Recipient::from_string(String::from("addr2")),
                weight: Uint128::new(60),
            },
            AddressWeight {
                recipient: Recipient::from_string(String::from("addr3")),
                weight: Uint128::new(50),
            },
            AddressWeight {
                recipient: Recipient::from_string(String::from("addr4")),
                weight: Uint128::new(100),
            },
        ],
        lock: Milliseconds::default(),
        default_recipient: None,
    };
    assert_eq!(expected_splitter, splitter);

    // check result
    let splitter = SPLITTER.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        splitter.recipients[3],
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr4")),
            weight: Uint128::new(100),
        }
    );
    assert_eq!(splitter.recipients.len(), 4);
}

#[test]
fn test_execute_add_recipient_duplicate_recipient() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(60),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr3")),
            weight: Uint128::new(50),
        },
    ];

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info(owner, &[]);

    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };
    let splitter = Splitter {
        recipients: recipient,
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // Works

    let msg = ExecuteMsg::AddRecipient {
        recipient: AddressWeight {
            recipient: Recipient::from_string(String::from("addr4")),
            weight: Uint128::new(100),
        },
    };

    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_eq!(
        Response::default().add_attributes(vec![attr("action", "added_recipient")]),
        res
    );
    // Add a duplicate user
    let msg = ExecuteMsg::AddRecipient {
        recipient: AddressWeight {
            recipient: Recipient::from_string(String::from("addr4")),
            weight: Uint128::new(100),
        },
    };
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(res, ContractError::DuplicateRecipient {});
}
#[test]
fn test_execute_add_recipient_invalid_weight() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(60),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr3")),
            weight: Uint128::new(50),
        },
    ];

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let info = mock_info(owner, &[]);

    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };
    let splitter = Splitter {
        recipients: recipient,
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // Invalid weight

    let msg = ExecuteMsg::AddRecipient {
        recipient: AddressWeight {
            recipient: Recipient::from_string(String::from("addr4")),
            weight: Uint128::zero(),
        },
    };

    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(ContractError::InvalidWeight {}, res);
}

#[test]
fn test_execute_add_recipient_locked_contract() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(60),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr3")),
            weight: Uint128::new(50),
        },
    ];
    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();
    let info = mock_info(owner, &[]);
    let current_time = env.block.time.seconds();
    let splitter = Splitter {
        recipients: recipient,
        lock: Milliseconds::default().plus_seconds(current_time + 1),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::ContractLocked {}, res.unwrap_err());
}

#[test]
fn test_execute_add_recipient_unauthorized() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(60),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr3")),
            weight: Uint128::new(50),
        },
    ];
    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient,
    };

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();
    let info = mock_info("incorrect_owner", &[]);
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
}

#[test]
fn test_execute_update_recipients() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let owner = "creator";

    let splitter = Splitter {
        recipients: vec![],
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(60),
        },
    ];
    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient.clone(),
    };
    let info = mock_info(owner, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        Response::default().add_attributes(vec![attr("action", "update_recipients")]),
        res
    );

    //check result
    let splitter = SPLITTER.load(deps.as_ref().storage).unwrap();
    assert_eq!(splitter.recipients, recipient);
}

#[test]
fn test_execute_update_recipients_invalid_weight() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::zero(),
        },
    ];
    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient,
    };

    let splitter = Splitter {
        recipients: vec![],
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    // Invalid weight

    let info = mock_info(owner, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(res, ContractError::InvalidWeight {});
}

#[test]
fn test_execute_update_recipients_contract_locked() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::new(100),
        },
    ];
    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient,
    };

    let current_time = env.block.time.seconds();

    let splitter = Splitter {
        recipients: vec![],
        lock: Milliseconds::default().plus_seconds(current_time + 10),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    // Invalid weight

    let info = mock_info(owner, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(res, ContractError::ContractLocked {});
}

#[test]
fn test_execute_update_recipients_unauthorized() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let owner = "creator";

    let recipient = vec![
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr1")),
            weight: Uint128::new(40),
        },
        AddressWeight {
            recipient: Recipient::from_string(String::from("addr2")),
            weight: Uint128::zero(),
        },
    ];
    let msg = ExecuteMsg::UpdateRecipients {
        recipients: recipient,
    };

    let splitter = Splitter {
        recipients: vec![],
        lock: Milliseconds::default(),
        default_recipient: None,
    };

    SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

    let deps_mut = deps.as_mut();
    ADOContract::default()
        .instantiate(
            deps_mut.storage,
            mock_env(),
            deps_mut.api,
            &deps_mut.querier,
            mock_info(owner, &[]),
            BaseInstantiateMsg {
                ado_type: "splitter".to_string(),
                ado_version: CONTRACT_VERSION.to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();

    // Unauthorized

    let info = mock_info("incorrect_owner", &[]);
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
}

#[test]
fn test_execute_send() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let _res: Response = init(deps.as_mut());

    let sender_funds_amount = 10000u128;

    let info = mock_info(OWNER, &[Coin::new(sender_funds_amount, "uluna")]);

    let recip_address1 = "address1".to_string();
    let recip_weight1 = 10; // 10%

    let recip_address2 = "address2".to_string();
    let recip_weight2 = 20; // 20%

    let recip_address3 = "address3".to_string();
    let recip_weight3 = 50; // 50%

    let recip1 = Recipient::from_string(recip_address1.clone());
    let recip2 = Recipient::from_string(recip_address2.clone());
    let recip3 = Recipient::from_string(recip_address3.clone());

    let config_recipient = vec![AddressWeight {
        recipient: recip3.clone(),
        weight: Uint128::new(recip_weight3),
    }];

    let recipient = vec![
        AddressWeight {
            recipient: recip1.clone(),
            weight: Uint128::new(recip_weight1),
        },
        AddressWeight {
            recipient: recip2.clone(),
            weight: Uint128::new(recip_weight2),
        },
    ];
    let msg = ExecuteMsg::Send { config: None };

    let splitter = Splitter {
        recipients: recipient,
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
                    to_address: recip_address1.clone(),
                    amount: vec![Coin::new(3333, "uluna")],
                }),
            ),
            SubMsg::new(
                // refunds remainder to sender
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: recip_address2.clone(),
                    amount: vec![Coin::new(6666, "uluna")],
                }),
            ),
            SubMsg::new(
                // refunds remainder to sender
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: OWNER.to_string(),
                    amount: vec![Coin::new(1, "uluna")],
                }),
            ),
        ])
        .add_attributes(vec![attr("action", "send"), attr("sender", "creator")]);

    assert_eq!(res, expected_res);

    // Test send with config
    let msg = ExecuteMsg::Send {
        config: Some(config_recipient),
    };
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    let expected_res = Response::new()
        .add_submessages(vec![
            SubMsg::new(
                // refunds remainder to sender
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: recip_address3,
                    amount: vec![Coin::new(10000, "uluna")],
                }),
            ),
            // amp_msg,
        ])
        .add_attributes(vec![attr("action", "send"), attr("sender", "creator")]);

    assert_eq!(res, expected_res);
}

// #[test]
// fn test_query_splitter() {
//     let mut deps = mock_dependencies_custom(&[]);
//     let env = mock_env();
//     let splitter = Splitter {
//         recipients: vec![],
//         lock: Milliseconds::default(),
//     };

//     SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

//     let query_msg = QueryMsg::GetSplitterConfig {};
//     let res = query(deps.as_ref(), env, query_msg).unwrap();
//     let val: GetSplitterConfigResponse = from_json(&res).unwrap();

//     assert_eq!(val.config, splitter);
// }

// #[test]
// fn test_query_user_weight() {
//     let mut deps = mock_dependencies_custom(&[]);
//     let env = mock_env();
//     let user1 = AddressWeight {
//         recipient: Recipient::Addr("first".to_string()),
//         weight: Uint128::new(5),
//     };
//     let user2 = AddressWeight {
//         recipient: Recipient::Addr("second".to_string()),
//         weight: Uint128::new(10),
//     };
//     let splitter = Splitter {
//         recipients: vec![user1, user2],
//         lock: Milliseconds::default(),
//     };

//     SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

//     let query_msg = QueryMsg::GetUserWeight {
//         user: Recipient::Addr("second".to_string()),
//     };
//     let res = query(deps.as_ref(), env, query_msg).unwrap();
//     let val: GetUserWeightResponse = from_json(&res).unwrap();

//     assert_eq!(val.weight, Uint128::new(10));
//     assert_eq!(val.total_weight, Uint128::new(15));
// }

// #[test]
// fn test_execute_send_error() {
//     // Send more than 5 coins
//     let mut deps = mock_dependencies_custom(&[]);
//     let env = mock_env();

//     let sender_funds_amount = 10000u128;
//     let owner = "creator";

//     let recip_address1 = "address1".to_string();
//     let recip_weight1 = Uint128::new(10); // Weight of 10

//     let recip_address2 = "address2".to_string();
//     let recip_weight2 = Uint128::new(20); // Weight of 20

//     let recipient = vec![
//         AddressWeight {
//             recipient: Recipient::Addr(recip_address1),
//             weight: recip_weight1,
//         },
//         AddressWeight {
//             recipient: Recipient::Addr(recip_address2),
//             weight: recip_weight2,
//         },
//     ];
//     let msg = ExecuteMsg::Send {
//         reply_gas_exit: None,
//         packet: None,
//     };

//     let info = mock_info(
//         owner,
//         &vec![
//             Coin::new(sender_funds_amount, "uluna"),
//             Coin::new(sender_funds_amount, "uluna"),
//             Coin::new(sender_funds_amount, "uluna"),
//             Coin::new(sender_funds_amount, "uluna"),
//             Coin::new(sender_funds_amount, "uluna"),
//             Coin::new(sender_funds_amount, "uluna"),
//         ],
//     );
//     let splitter = Splitter {
//         recipients: recipient.clone(),
//         lock: Milliseconds::default(),
//     };

//     SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

//     let res = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap_err();

//     let expected_res = ContractError::ExceedsMaxAllowedCoins {};

//     assert_eq!(res, expected_res);

//     // Send 0 coins
//     let info = mock_info(owner, &[]);
//     let splitter = Splitter {
//         recipients: recipient,
//         lock: Milliseconds::default(),
//     };

//     SPLITTER.save(deps.as_mut().storage, &splitter).unwrap();

//     let res = execute(deps.as_mut(), env, info, msg).unwrap_err();

//     let expected_res = ContractError::InvalidFunds {
//         msg: "ensure! at least one coin to be sent".to_string(),
//     };

//     assert_eq!(res, expected_res);
// }
