use super::{contract::*, state::ADO_ADDRESSES};
use crate::state::{ADO_DESCRIPTORS, ADO_IDX};
use andromeda_app::app::{AppComponent, ComponentType, ExecuteMsg, InstantiateMsg};
use andromeda_std::ado_base::ownership::OwnershipMessage;
use andromeda_std::testing::mock_querier::{
    mock_dependencies_custom, MOCK_ANCHOR_CONTRACT, MOCK_CW20_CONTRACT, MOCK_KERNEL_CONTRACT,
};
use andromeda_std::{ado_base::AndromedaMsg, error::ContractError};
use cosmwasm_std::{
    attr,
    testing::{mock_env, mock_info},
    to_json_binary, Addr, CosmosMsg, Empty, ReplyOn, Response, StdError, SubMsg, WasmMsg,
};
use cosmwasm_std::{Binary, Event, Reply, SubMsgResponse, SubMsgResult};

#[test]
fn test_empty_instantiation() {
    let mut deps = mock_dependencies_custom(&[]);

    let msg = InstantiateMsg {
        app_components: vec![],
        name: String::from("Some App"),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
        chain_info: None,
    };
    let info = mock_info("creator", &[]);

    // we can just call .unwrap() to assert this was a success
    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(1, res.messages.len());
}

//TODO: Fix post CosmWasm 2.0

// #[test]
// fn test_instantiation() {
//     let mut deps = mock_dependencies_custom(&[]);

//     let msg = InstantiateMsg {
//         app_components: vec![AppComponent {
//             name: "token".to_string(),
//             ado_type: "cw721".to_string(),
//             component_type: ComponentType::New(to_json_binary(&true).unwrap()),
//         }],
//         name: String::from("Some App"),
//         kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
//         owner: None,
//         chain_info: None,
//     };
//     let info = mock_info("creator", &[]);

//     let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
//     assert_eq!(3, res.messages.len());
//     let inst_submsg: SubMsg<Empty> = SubMsg {
//         id: 1,
//         msg: CosmosMsg::Wasm(WasmMsg::Instantiate {
//             code_id: 1,
//             msg: to_json_binary(&true).unwrap(),
//             funds: vec![],
//             label: "Instantiate: cw721".to_string(),
//             admin: Some("creator".to_string()),
//         }),
//         reply_on: ReplyOn::Always,
//         gas_limit: None,
//     };
//     let sender = info.sender;
//     let register_submsg: SubMsg<Empty> = SubMsg {
//         id: ReplyId::RegisterPath.repr(),
//         msg: CosmosMsg::Wasm(WasmMsg::Execute {
//             contract_addr: "vfs_contract".to_string(),
//             msg: to_json_binary(&VFSExecuteMsg::AddChild {
//                 name: convert_component_name("Some App"),
//                 parent_address: AndrAddr::from_string(format!("{sender}")),
//             })
//             .unwrap(),
//             funds: vec![],
//         }),
//         reply_on: ReplyOn::Error,
//         gas_limit: None,
//     };
//     let assign_msg: SubMsg<Empty> = SubMsg {
//         id: ReplyId::AssignApp.repr(),
//         msg: CosmosMsg::Wasm(WasmMsg::Execute {
//             contract_addr: "cosmos2contract".to_string(),
//             msg: to_json_binary(&ExecuteMsg::AssignAppToComponents {}).unwrap(),
//             funds: vec![],
//         }),
//         reply_on: ReplyOn::Error,
//         gas_limit: None,
//     };
//     let expected = Response::new()
//         .add_submessage(register_submsg)
//         .add_submessage(inst_submsg)
//         .add_submessage(assign_msg)
//         .add_attributes(vec![
//             attr("method", "instantiate"),
//             attr("type", "app-contract"),
//             attr("owner", "creator"),
//             attr("andr_app", "Some App"),
//         ]);

//     assert_eq!(expected, res);

//     assert_eq!(
//         Addr::unchecked(""),
//         ADO_ADDRESSES.load(deps.as_ref().storage, "token").unwrap()
//     );
// }

// #[test]
// fn test_instantiation_duplicate_components() {
//     let mut deps = mock_dependencies_custom(&[]);

//     let msg = InstantiateMsg {
//         app_components: vec![
//             AppComponent {
//                 name: "component".to_string(),
//                 ado_type: "cw721".to_string(),
//                 component_type: ComponentType::New(to_json_binary(&true).unwrap()),
//             },
//             AppComponent {
//                 name: "component".to_string(),
//                 ado_type: "cw20".to_string(),
//                 component_type: ComponentType::New(to_json_binary(&true).unwrap()),
//             },
//         ],
//         name: String::from("Some App"),
//         kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
//         owner: None,
//         chain_info: None,
//     };
//     let info = mock_info("creator", &[]);

//     let res = instantiate(deps.as_mut(), mock_env(), info, msg);
//     assert_eq!(ContractError::NameAlreadyTaken {}, res.unwrap_err());
// }

#[test]
fn test_add_app_component_unauthorized() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);
    let inst_msg = InstantiateMsg {
        app_components: vec![],
        name: String::from("Some App"),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
        chain_info: None,
    };

    instantiate(deps.as_mut(), env.clone(), info, inst_msg).unwrap();

    let unauth_info = mock_info("anyone", &[]);
    let msg = ExecuteMsg::AddAppComponent {
        component: AppComponent {
            name: "token".to_string(),
            ado_type: "cw721".to_string(),
            component_type: ComponentType::New(to_json_binary(&true).unwrap()),
        },
    };

    let err = execute(deps.as_mut(), env, unauth_info, msg).unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, err);
}

// #[test]
// fn test_add_app_component_duplicate_name() {
//     let mut deps = mock_dependencies_custom(&[]);
//     let env = mock_env();
//     let info = mock_info("creator", &[]);
//     let inst_msg = InstantiateMsg {
//         app_components: vec![AppComponent {
//             name: "token".to_string(),
//             ado_type: "cw721".to_string(),
//             component_type: ComponentType::New(to_json_binary(&true).unwrap()),
//         }],
//         name: String::from("Some App"),
//         kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
//         owner: None,
//         chain_info: None,
//     };

//     instantiate(deps.as_mut(), env.clone(), info.clone(), inst_msg).unwrap();
//     ADO_ADDRESSES
//         .save(
//             deps.as_mut().storage,
//             "token",
//             &Addr::unchecked("someaddress"),
//         )
//         .unwrap();

//     let msg = ExecuteMsg::AddAppComponent {
//         component: AppComponent {
//             name: "token".to_string(),
//             ado_type: "cw721".to_string(),
//             component_type: ComponentType::New(to_json_binary(&true).unwrap()),
//         },
//     };

//     let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
//     assert_eq!(ContractError::NameAlreadyTaken {}, err);
// }

// #[test]
// fn test_add_app_component() {
//     let mut deps = mock_dependencies_custom(&[]);
//     let env = mock_env();
//     let info = mock_info("creator", &[]);
//     let inst_msg = InstantiateMsg {
//         app_components: vec![],
//         name: String::from("Some App"),
//         kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
//         owner: None,
//         chain_info: None,
//     };

//     instantiate(deps.as_mut(), env.clone(), info.clone(), inst_msg).unwrap();

//     let msg = ExecuteMsg::AddAppComponent {
//         component: AppComponent {
//             name: "token".to_string(),
//             ado_type: "cw721".to_string(),
//             component_type: ComponentType::New(to_json_binary(&true).unwrap()),
//         },
//     };

//     let res = execute(deps.as_mut(), env, info, msg).unwrap();
//     assert_eq!(1, res.messages.len());
//     // let inst_submsg: SubMsg<Empty> = SubMsg {
//     //     id: 1,
//     //     msg: CosmosMsg::Wasm(WasmMsg::Instantiate {
//     //         code_id: 1,
//     //         msg: to_json_binary(&true).unwrap(),
//     //         funds: vec![],
//     //         label: "Instantiate: cw721".to_string(),
//     //         admin: Some("creator".to_string()),
//     //     }),
//     //     reply_on: ReplyOn::Always,
//     //     gas_limit: None,
//     // };
//     // let expected = Response::new()
//     //     .add_submessage(inst_submsg)
//     //     .add_attributes(vec![
//     //         attr("method", "add_app_component"),
//     //         attr("name", "token"),
//     //         attr("type", "cw721"),
//     //     ]);

//     // assert_eq!(expected, res);

//     // assert_eq!(
//     //     Addr::unchecked(""),
//     //     ADO_ADDRESSES.load(deps.as_ref().storage, "token").unwrap()
//     // );
// }

#[test]
fn test_claim_ownership_unauth() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);
    let inst_msg = InstantiateMsg {
        app_components: vec![],
        name: String::from("Some App"),
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        chain_info: None,
    };

    instantiate(deps.as_mut(), env.clone(), info, inst_msg).unwrap();

    let unauth_info = mock_info("anyone", &[]);
    let msg = ExecuteMsg::ClaimOwnership {
        name: None,
        new_owner: None,
    };

    let err = execute(deps.as_mut(), env, unauth_info, msg).unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, err);
}

#[test]
fn test_claim_ownership_not_found() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);
    let inst_msg = InstantiateMsg {
        app_components: vec![],
        name: String::from("Some App"),
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        chain_info: None,
    };

    instantiate(deps.as_mut(), env.clone(), info.clone(), inst_msg).unwrap();

    let msg = ExecuteMsg::ClaimOwnership {
        name: Some("token".to_string()),
        new_owner: None,
    };

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert!(matches!(err, ContractError::Std(StdError::NotFound { .. })));
}

#[test]
fn test_claim_ownership_empty() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);
    let inst_msg = InstantiateMsg {
        app_components: vec![],
        name: String::from("Some App"),
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        chain_info: None,
    };

    instantiate(deps.as_mut(), env.clone(), info.clone(), inst_msg).unwrap();

    let msg = ExecuteMsg::ClaimOwnership {
        name: None,
        new_owner: None,
    };

    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(0, res.messages.len());
}

#[test]
fn test_claim_ownership_all() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);
    let inst_msg = InstantiateMsg {
        app_components: vec![],
        name: String::from("Some App"),
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        chain_info: None,
    };

    instantiate(deps.as_mut(), env.clone(), info.clone(), inst_msg).unwrap();
    ADO_ADDRESSES
        .save(
            deps.as_mut().storage,
            "token",
            &Addr::unchecked(MOCK_CW20_CONTRACT),
        )
        .unwrap();
    ADO_ADDRESSES
        .save(
            deps.as_mut().storage,
            "anchor",
            &Addr::unchecked(MOCK_ANCHOR_CONTRACT),
        )
        .unwrap();

    let msg = ExecuteMsg::ClaimOwnership {
        name: None,
        new_owner: None,
    };

    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(2, res.messages.len());
}

#[test]
fn test_claim_ownership() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);
    let inst_msg = InstantiateMsg {
        app_components: vec![],
        name: String::from("Some App"),
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        chain_info: None,
    };

    instantiate(deps.as_mut(), env.clone(), info.clone(), inst_msg).unwrap();
    ADO_ADDRESSES
        .save(
            deps.as_mut().storage,
            "token",
            &Addr::unchecked("tokenaddress".to_string()),
        )
        .unwrap();
    ADO_ADDRESSES
        .save(
            deps.as_mut().storage,
            "anchor",
            &Addr::unchecked("anchoraddress".to_string()),
        )
        .unwrap();

    let msg = ExecuteMsg::ClaimOwnership {
        name: Some("token".to_string()),
        new_owner: None,
    };

    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(1, res.messages.len());

    let exec_submsg: SubMsg<Empty> = SubMsg {
        id: 200,
        msg: CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "tokenaddress".to_string(),
            msg: to_json_binary(&AndromedaMsg::Ownership(OwnershipMessage::UpdateOwner {
                new_owner: Addr::unchecked("creator"),
                expiration: None,
            }))
            .unwrap(),
            funds: vec![],
        }),
        reply_on: ReplyOn::Error,
        gas_limit: None,
    };
    let expected = Response::new()
        .add_submessage(exec_submsg)
        .add_attributes(vec![attr("method", "claim_ownership")]);

    assert_eq!(expected, res)
}

#[test]
fn test_proxy_message_unauth() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);
    let inst_msg = InstantiateMsg {
        app_components: vec![],
        name: String::from("Some App"),
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        chain_info: None,
    };

    instantiate(deps.as_mut(), env.clone(), info, inst_msg).unwrap();

    let unauth_info = mock_info("anyone", &[]);
    let msg = ExecuteMsg::ProxyMessage {
        name: "token".to_string(),
        msg: to_json_binary(&true).unwrap(),
    };

    let err = execute(deps.as_mut(), env, unauth_info, msg).unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, err);
}

#[test]
fn test_proxy_message_not_found() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);
    let inst_msg = InstantiateMsg {
        app_components: vec![],
        name: String::from("Some App"),
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        chain_info: None,
    };

    instantiate(deps.as_mut(), env.clone(), info.clone(), inst_msg).unwrap();

    let msg = ExecuteMsg::ProxyMessage {
        name: "token".to_string(),
        msg: to_json_binary(&true).unwrap(),
    };

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();

    assert!(matches!(err, ContractError::Std(StdError::NotFound { .. })));
}

#[test]
fn test_proxy_message() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);
    let inst_msg = InstantiateMsg {
        app_components: vec![],
        name: String::from("Some App"),
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        chain_info: None,
    };
    ADO_ADDRESSES
        .save(
            deps.as_mut().storage,
            "token",
            &Addr::unchecked("tokenaddress".to_string()),
        )
        .unwrap();
    instantiate(deps.as_mut(), env.clone(), info.clone(), inst_msg).unwrap();

    let msg = ExecuteMsg::ProxyMessage {
        name: "token".to_string(),
        msg: to_json_binary(&true).unwrap(),
    };

    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    let exec_submsg: SubMsg<Empty> = SubMsg {
        id: 102,
        msg: CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "tokenaddress".to_string(),
            msg: to_json_binary(&true).unwrap(),
            funds: vec![],
        }),
        reply_on: ReplyOn::Error,
        gas_limit: None,
    };
    let expected = Response::new()
        .add_submessage(exec_submsg)
        .add_attributes(vec![
            attr("method", "app_message"),
            attr("recipient", "token"),
        ]);

    assert_eq!(expected, res)
}

#[test]
fn test_update_address_unauth() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);
    let inst_msg = InstantiateMsg {
        app_components: vec![],
        name: String::from("Some App"),
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        chain_info: None,
    };

    ADO_ADDRESSES
        .save(
            deps.as_mut().storage,
            "token",
            &Addr::unchecked("tokenaddress".to_string()),
        )
        .unwrap();
    instantiate(deps.as_mut(), env.clone(), info, inst_msg).unwrap();

    let unauth_info = mock_info("anyone", &[]);
    let msg = ExecuteMsg::UpdateAddress {
        name: "token".to_string(),
        addr: "newtokenaddress".to_string(),
    };

    let err = execute(deps.as_mut(), env, unauth_info, msg).unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, err);
}

#[test]
fn test_update_address_not_found() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);
    let inst_msg = InstantiateMsg {
        app_components: vec![],
        name: String::from("Some App"),
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        chain_info: None,
    };

    instantiate(deps.as_mut(), env.clone(), info.clone(), inst_msg).unwrap();

    let msg = ExecuteMsg::UpdateAddress {
        name: "token".to_string(),
        addr: "newtokenaddress".to_string(),
    };

    let res = execute(deps.as_mut(), env, info, msg);
    assert!(matches!(
        res,
        Err(ContractError::Std(StdError::NotFound { .. }))
    ));
}

#[test]
fn test_update_address() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);
    let inst_msg = InstantiateMsg {
        app_components: vec![],
        name: String::from("Some App"),
        owner: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        chain_info: None,
    };

    ADO_ADDRESSES
        .save(
            deps.as_mut().storage,
            "token",
            &Addr::unchecked("tokenaddress".to_string()),
        )
        .unwrap();
    instantiate(deps.as_mut(), env.clone(), info.clone(), inst_msg).unwrap();

    let msg = ExecuteMsg::UpdateAddress {
        name: "token".to_string(),
        addr: "newtokenaddress".to_string(),
    };

    execute(deps.as_mut(), env, info, msg).unwrap();

    let addr = ADO_ADDRESSES.load(deps.as_ref().storage, "token").unwrap();
    assert_eq!(Addr::unchecked("newtokenaddress"), addr)
}

#[test]
fn test_add_app_component_limit() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);

    let msg = InstantiateMsg {
        app_components: vec![],
        name: String::from("Some App"),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
        chain_info: None,
    };

    // we can just call .unwrap() to assert this was a success
    instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let mut i = 0;
    while i < 50 {
        i += 1;
        ADO_ADDRESSES
            .save(deps.as_mut().storage, &i.to_string(), &Addr::unchecked(""))
            .unwrap();
    }
    ADO_IDX.save(deps.as_mut().storage, &50).unwrap();

    let msg = ExecuteMsg::AddAppComponent {
        component: AppComponent {
            name: "token".to_string(),
            ado_type: "cw721".to_string(),
            component_type: ComponentType::New(to_json_binary(&true).unwrap()),
        },
    };

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(ContractError::TooManyAppComponents {}, err);
}

#[test]
fn test_reply_assign_app() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let mock_app_component = AppComponent {
        ado_type: "cw721".to_string(),
        name: "token".to_string(),
        component_type: ComponentType::New(to_json_binary(&true).unwrap()),
    };
    let component_idx = 1;
    ADO_DESCRIPTORS
        .save(
            deps.as_mut().storage,
            &component_idx.to_string(),
            &mock_app_component,
        )
        .unwrap();
    ADO_ADDRESSES
        .save(
            deps.as_mut().storage,
            &mock_app_component.name,
            &Addr::unchecked("cosmos2contract"),
        )
        .unwrap();

    let mock_reply_event = Event::new("instantiate").add_attribute(
        "contract_address".to_string(),
        "cosmos2contract".to_string(),
    );

    let reply_resp = "Cg9jb3Ntb3MyY29udHJhY3QSAA==";
    let mock_reply = Reply {
        id: component_idx,
        result: SubMsgResult::Ok(SubMsgResponse {
            data: Some(Binary::from_base64(reply_resp).unwrap()),
            events: vec![mock_reply_event],
        }),
    };

    let res = reply(deps.as_mut(), env, mock_reply).unwrap();
    assert!(res.messages.is_empty());
}
