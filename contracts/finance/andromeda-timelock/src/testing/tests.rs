use andromeda_std::{
    ado_base::Module,
    amp::{AndrAddr, Recipient},
    error::ContractError,
    testing::mock_querier::{MOCK_ADDRESS_LIST_CONTRACT, MOCK_KERNEL_CONTRACT},
};
use cosmwasm_std::{
    attr, coin, coins, from_binary,
    testing::{mock_env, mock_info},
    BankMsg, Coin, DepsMut, Response, StdError, Timestamp,
};
use cw_utils::Expiration;

use crate::{
    contract::{execute, instantiate, query},
    testing::mock_querier::mock_dependencies_custom,
};
use andromeda_finance::timelock::{
    Escrow, EscrowCondition, ExecuteMsg, GetLockedFundsResponse, InstantiateMsg, QueryMsg,
};

fn init(deps: DepsMut, _modules: Option<Vec<Module>>) -> Response {
    let modules = vec![Module {
        name: Some("address_list".to_string()),
        is_mutable: false,
        address: AndrAddr::from_string(MOCK_ADDRESS_LIST_CONTRACT.to_owned()),
    }];

    let msg = InstantiateMsg {
        owner: None,
        modules: Some(modules),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
    };
    let info = mock_info("owner", &[]);
    instantiate(deps, mock_env(), info, msg).unwrap()
}

#[test]
fn test_modules() {
    let mut deps = mock_dependencies_custom(&[]);
    let res = init(deps.as_mut(), None);

    assert_eq!(
        Response::new()
            .add_attribute("method", "instantiate")
            .add_attribute("type", "timelock")
            .add_attribute("action", "register_module")
            .add_attribute("module_idx", "1"),
        res
    );

    let msg = ExecuteMsg::HoldFunds {
        condition: None,
        recipient: None,
    };
    let info = mock_info("anyone", &coins(100, "uusd"));
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone());
    assert_eq!(
        ContractError::Std(StdError::generic_err(
            "Querier contract error: InvalidAddress"
        )),
        res.unwrap_err()
    );

    let info = mock_info("sender", &coins(100, "uusd"));
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    // assert_eq!(
    //     Response::new()
    //         .add_attribute("action", "hold_funds")
    //         .add_attribute("sender", "sender")
    //         .add_attribute("recipient", "Recipient { address: AndrAddr(\"sender\")")
    //         .add_attribute("condition", "None"),
    //     res.
    // );
}

// #[test]
// fn test_update_app_contract() {
//     let mut deps = mock_dependencies_custom(&[]);

//     let info = mock_info("app_contract", &[]);
//     let _res = init(deps.as_mut(), None);

//     let msg = ExecuteMsg::UpdateAppContract {
//         address: "app_contract".to_string(),
//     };
//     let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
//     assert_eq!(
//         Response::new()
//             .add_attribute("action", "update_app_contract")
//             .add_attribute("address", "app_contract"),
//         res
//     );
// }

#[test]
fn test_execute_hold_funds() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let owner = "owner";
    let funds = vec![Coin::new(1000, "uusd")];
    let condition = EscrowCondition::Expiration(Expiration::AtHeight(1));
    let info = mock_info(owner, &funds);

    let msg = ExecuteMsg::HoldFunds {
        condition: Some(condition.clone()),
        recipient: None,
    };
    env.block.height = 0;

    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    let expected = Response::default().add_attributes(vec![
        attr("action", "hold_funds"),
        attr("sender", info.sender.to_string()),
        attr(
            "recipient",
            format!("{:?}", Recipient::from_string(info.sender.to_string())),
        ),
        attr("condition", format!("{:?}", Some(condition.clone()))),
    ]);
    assert_eq!(expected, res);

    let query_msg = QueryMsg::GetLockedFunds {
        owner: owner.to_string(),
        recipient: owner.to_string(),
    };

    let res = query(deps.as_ref(), env, query_msg).unwrap();
    let val: GetLockedFundsResponse = from_binary(&res).unwrap();
    let expected = Escrow {
        coins: funds,
        condition: Some(condition),
        recipient: Recipient::from_string(owner.to_string()),
        recipient_addr: owner.to_string(),
    };

    assert_eq!(val.funds.unwrap(), expected);
}

#[test]
fn test_execute_hold_funds_escrow_updated() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();

    let owner = "owner";
    let info = mock_info(owner, &coins(100, "uusd"));

    let msg = ExecuteMsg::HoldFunds {
        condition: Some(EscrowCondition::Expiration(Expiration::AtHeight(10))),
        recipient: Some(Recipient::from_string("recipient".to_string())),
    };

    env.block.height = 0;

    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let msg = ExecuteMsg::HoldFunds {
        condition: Some(EscrowCondition::Expiration(Expiration::AtHeight(100))),
        recipient: Some(Recipient::from_string("recipient".to_string())),
    };

    env.block.height = 120;

    let info = mock_info(owner, &[coin(100, "uusd"), coin(100, "uluna")]);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    let query_msg = QueryMsg::GetLockedFunds {
        owner: owner.to_string(),
        recipient: "recipient".to_string(),
    };

    let res = query(deps.as_ref(), env, query_msg).unwrap();
    let val: GetLockedFundsResponse = from_binary(&res).unwrap();
    let expected = Escrow {
        // Coins get merged.
        coins: vec![coin(200, "uusd"), coin(100, "uluna")],
        // Original expiration remains.
        condition: Some(EscrowCondition::Expiration(Expiration::AtHeight(10))),
        recipient: Recipient::from_string("recipient".to_string()),
        recipient_addr: "recipient".to_string(),
    };

    assert_eq!(val.funds.unwrap(), expected);
}

#[test]
fn test_execute_release_funds_block_condition() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let owner = "owner";

    let info = mock_info(owner, &[coin(100, "uusd")]);
    let msg = ExecuteMsg::HoldFunds {
        condition: Some(EscrowCondition::Expiration(Expiration::AtHeight(1))),
        recipient: None,
    };
    env.block.height = 0;
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    env.block.height = 2;
    let msg = ExecuteMsg::ReleaseFunds {
        recipient_addr: None,
        start_after: None,
        limit: None,
    };
    let res = execute(deps.as_mut(), env, info.clone(), msg).unwrap();
    let bank_msg = BankMsg::Send {
        to_address: "owner".into(),
        amount: info.funds,
    };
    assert_eq!(
        Response::new().add_message(bank_msg).add_attributes(vec![
            attr("action", "release_funds"),
            attr("recipient_addr", "owner"),
        ]),
        res
    );
}

#[test]
fn test_execute_release_funds_no_condition() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let owner = "owner";

    let info = mock_info(owner, &[coin(100, "uusd")]);
    let msg = ExecuteMsg::HoldFunds {
        condition: None,
        recipient: None,
    };
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::ReleaseFunds {
        recipient_addr: None,
        start_after: None,
        limit: None,
    };
    let res = execute(deps.as_mut(), env, info.clone(), msg).unwrap();
    let bank_msg = BankMsg::Send {
        to_address: "owner".into(),
        amount: info.funds,
    };
    assert_eq!(
        Response::new().add_message(bank_msg).add_attributes(vec![
            attr("action", "release_funds"),
            attr("recipient_addr", "owner"),
        ]),
        res
    );
}

#[test]
fn test_execute_release_multiple_escrows() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let recipient = Recipient::from_string("recipient".to_string());

    let msg = ExecuteMsg::HoldFunds {
        condition: None,
        recipient: Some(recipient),
    };
    let info = mock_info("sender1", &coins(100, "uusd"));
    let _res = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap();

    let info = mock_info("sender2", &coins(200, "uusd"));
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::ReleaseFunds {
        recipient_addr: Some("recipient".into()),
        start_after: None,
        limit: None,
    };

    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    let bank_msg1 = BankMsg::Send {
        to_address: "recipient".into(),
        amount: coins(100, "uusd"),
    };
    let bank_msg2 = BankMsg::Send {
        to_address: "recipient".into(),
        amount: coins(200, "uusd"),
    };
    assert_eq!(
        Response::new()
            .add_messages(vec![bank_msg1, bank_msg2])
            .add_attributes(vec![
                attr("action", "release_funds"),
                attr("recipient_addr", "recipient"),
            ]),
        res
    );
}

#[test]
fn test_execute_release_funds_time_condition() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let owner = "owner";

    let info = mock_info(owner, &[coin(100, "uusd")]);
    let msg = ExecuteMsg::HoldFunds {
        condition: Some(EscrowCondition::Expiration(Expiration::AtTime(
            Timestamp::from_seconds(100),
        ))),
        recipient: None,
    };
    env.block.time = Timestamp::from_seconds(50);
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::ReleaseFunds {
        recipient_addr: None,
        start_after: None,
        limit: None,
    };

    env.block.time = Timestamp::from_seconds(150);
    let res = execute(deps.as_mut(), env, info.clone(), msg).unwrap();
    let bank_msg = BankMsg::Send {
        to_address: "owner".into(),
        amount: info.funds,
    };
    assert_eq!(
        Response::new().add_message(bank_msg).add_attributes(vec![
            attr("action", "release_funds"),
            attr("recipient_addr", "owner"),
        ]),
        res
    );
}

#[test]
fn test_execute_release_funds_locked() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let owner = "owner";

    let info = mock_info(owner, &[coin(100, "uusd")]);
    let msg = ExecuteMsg::HoldFunds {
        condition: Some(EscrowCondition::Expiration(Expiration::AtTime(
            Timestamp::from_seconds(100),
        ))),
        recipient: None,
    };
    env.block.time = Timestamp::from_seconds(50);
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::ReleaseFunds {
        recipient_addr: None,
        start_after: None,
        limit: None,
    };

    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::FundsAreLocked {}, res.unwrap_err());
}

#[test]
fn test_execute_release_funds_min_funds_condition() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let owner = "owner";

    let info = mock_info(owner, &[coin(100, "uusd")]);
    let msg = ExecuteMsg::HoldFunds {
        condition: Some(EscrowCondition::MinimumFunds(vec![
            coin(200, "uusd"),
            coin(100, "uluna"),
        ])),
        recipient: None,
    };
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::ReleaseFunds {
        recipient_addr: None,
        start_after: None,
        limit: None,
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(ContractError::FundsAreLocked {}, res.unwrap_err());

    // Update the escrow with enough funds.
    let msg = ExecuteMsg::HoldFunds {
        condition: None,
        recipient: None,
    };
    let info = mock_info(owner, &[coin(110, "uusd"), coin(120, "uluna")]);
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // Now try to release funds.
    let msg = ExecuteMsg::ReleaseFunds {
        recipient_addr: None,
        start_after: None,
        limit: None,
    };

    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    let bank_msg = BankMsg::Send {
        to_address: "owner".into(),
        amount: vec![coin(210, "uusd"), coin(120, "uluna")],
    };
    assert_eq!(
        Response::new().add_message(bank_msg).add_attributes(vec![
            attr("action", "release_funds"),
            attr("recipient_addr", "owner"),
        ]),
        res
    );
}

#[test]
fn test_execute_release_specific_funds_no_funds_locked() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let owner = "owner";

    let info = mock_info(owner, &[]);
    let msg = ExecuteMsg::ReleaseSpecificFunds {
        recipient_addr: None,
        owner: owner.into(),
    };
    let res = execute(deps.as_mut(), env, info, msg);
    assert_eq!(ContractError::NoLockedFunds {}, res.unwrap_err());
}

#[test]
fn test_execute_release_specific_funds_no_condition() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let owner = "owner";

    let info = mock_info(owner, &[coin(100, "uusd")]);
    let msg = ExecuteMsg::HoldFunds {
        condition: None,
        recipient: None,
    };
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::ReleaseSpecificFunds {
        recipient_addr: None,
        owner: owner.into(),
    };
    let res = execute(deps.as_mut(), env, info.clone(), msg).unwrap();
    let bank_msg = BankMsg::Send {
        to_address: "owner".into(),
        amount: info.funds,
    };
    assert_eq!(
        Response::new().add_message(bank_msg).add_attributes(vec![
            attr("action", "release_funds"),
            attr("recipient_addr", "owner"),
        ]),
        res
    );
}

#[test]
fn test_execute_release_specific_funds_time_condition() {
    let mut deps = mock_dependencies_custom(&[]);
    let mut env = mock_env();
    let owner = "owner";

    let info = mock_info(owner, &[coin(100, "uusd")]);
    let msg = ExecuteMsg::HoldFunds {
        condition: Some(EscrowCondition::Expiration(Expiration::AtTime(
            Timestamp::from_seconds(100),
        ))),
        recipient: None,
    };
    env.block.time = Timestamp::from_seconds(50);
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::ReleaseSpecificFunds {
        recipient_addr: None,
        owner: owner.into(),
    };

    env.block.time = Timestamp::from_seconds(150);
    let res = execute(deps.as_mut(), env, info.clone(), msg).unwrap();
    let bank_msg = BankMsg::Send {
        to_address: "owner".into(),
        amount: info.funds,
    };
    assert_eq!(
        Response::new().add_message(bank_msg).add_attributes(vec![
            attr("action", "release_funds"),
            attr("recipient_addr", "owner"),
        ]),
        res
    );
}

#[test]
fn test_execute_release_specific_funds_min_funds_condition() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let owner = "owner";

    let info = mock_info(owner, &[coin(100, "uusd")]);
    let msg = ExecuteMsg::HoldFunds {
        condition: Some(EscrowCondition::MinimumFunds(vec![
            coin(200, "uusd"),
            coin(100, "uluna"),
        ])),
        recipient: None,
    };
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::ReleaseSpecificFunds {
        recipient_addr: None,
        owner: owner.into(),
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg);
    assert_eq!(ContractError::FundsAreLocked {}, res.unwrap_err());

    // Update the escrow with enough funds.
    let msg = ExecuteMsg::HoldFunds {
        condition: None,
        recipient: None,
    };
    let info = mock_info(owner, &[coin(110, "uusd"), coin(120, "uluna")]);
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // Now try to release funds.
    let msg = ExecuteMsg::ReleaseSpecificFunds {
        recipient_addr: None,
        owner: owner.into(),
    };

    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    let bank_msg = BankMsg::Send {
        to_address: "owner".into(),
        amount: vec![coin(210, "uusd"), coin(120, "uluna")],
    };
    assert_eq!(
        Response::new().add_message(bank_msg).add_attributes(vec![
            attr("action", "release_funds"),
            attr("recipient_addr", "owner"),
        ]),
        res
    );
}

// #[test]
// fn test_execute_receive() {
//     let mut deps = mock_dependencies_custom(&[]);
//     let env = mock_env();
//     let owner = "owner";
//     let funds = vec![Coin::new(1000, "uusd")];
//     let info = mock_info(owner, &funds);

//     let msg_struct = ExecuteMsg::HoldFunds {
//         condition: None,
//         recipient: None,
//     };
//     let msg_string = encode_binary(&msg_struct).unwrap();

//     let msg = ExecuteMsg::Receive(Some(msg_string));

//     let received = execute(deps.as_mut(), env, info.clone(), msg).unwrap();
//     let expected = Response::default().add_attributes(vec![
//         attr("action", "hold_funds"),
//         attr("sender", info.sender.to_string()),
//         attr("recipient", "Addr(\"owner\")"),
//         attr("condition", "None"),
//     ]);

//     assert_eq!(expected, received)
// }