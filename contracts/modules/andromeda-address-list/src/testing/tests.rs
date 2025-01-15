use crate::{
    contract::{execute, instantiate, query},
    state::PERMISSIONS,
    testing::mock_querier::{mock_dependencies_custom, MOCK_KERNEL_CONTRACT},
};
use andromeda_modules::address_list::{
    ActorPermission, ActorPermissionResponse, ExecuteMsg, IncludesActorResponse, InstantiateMsg,
    QueryMsg,
};
use andromeda_std::{
    ado_base::permissioning::LocalPermission, amp::AndrAddr, error::ContractError,
};
use cosmwasm_std::{
    attr, from_json,
    testing::{mock_env, mock_info},
    Addr, DepsMut, MessageInfo, Response,
};

fn init(deps: DepsMut, info: MessageInfo) {
    instantiate(
        deps,
        mock_env(),
        info,
        InstantiateMsg {
            kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
            owner: None,
            actor_permission: Some(ActorPermission {
                actors: vec![AndrAddr::from_string("actor")],
                permission: LocalPermission::whitelisted(None),
            }),
        },
    )
    .unwrap();
}

#[test]
fn test_instantiate() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info("creator", &[]);

    init(deps.as_mut(), info);
}

// #[test]
// fn test_instantiate_contract_permission() {
//     let mut deps = mock_dependencies_custom(&[]);
//     let info = mock_info("creator", &[]);

//     let err = instantiate(
//         deps.as_mut(),
//         mock_env(),
//         info,
//         InstantiateMsg {
//             kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
//             owner: None,
//             actor_permission: Some(ActorPermission {
//                 actor: Addr::unchecked(MOCK_KERNEL_CONTRACT),
//                 permission: Permission::Whitelisted(None),
//             }),
//         },
//     )
//     .unwrap_err();
//     assert_eq!(
//         err,
//         ContractError::InvalidPermission {
//             msg: "Contract permissions aren't allowed in the address list contract".to_string()
//         }
//     )
// }

#[test]
fn test_add_remove_actor() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let operator = "creator";
    let info = mock_info(operator, &[]);

    let actor = Addr::unchecked("actor");
    let permission = LocalPermission::default();

    init(deps.as_mut(), info.clone());

    let msg = ExecuteMsg::PermissionActors {
        actors: vec![AndrAddr::from_string(actor.clone())],
        permission: permission.clone(),
    };

    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
    let expected = Response::default().add_attributes(vec![
        attr("action", "add_actor_permission"),
        attr("actor", actor.clone()),
        attr("permission", permission.to_string()),
    ]);
    assert_eq!(expected, res);

    // Check that the actor and permission have been saved.
    let new_permission = PERMISSIONS.load(deps.as_ref().storage, &actor).unwrap();
    assert_eq!(new_permission, permission);

    // Try with unauthorized address
    let unauth_info = mock_info("anyone", &[]);
    let res = execute(deps.as_mut(), env.clone(), unauth_info, msg).unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, res);

    // // Contract permissions aren't allowed to be saved in the address list contract
    // let contract_permission = Permission::Whitelisted(None);
    // let msg = ExecuteMsg::PermissionActors {
    //     actor: Addr::unchecked(MOCK_KERNEL_CONTRACT),
    //     permission: contract_permission,
    // };
    // let err = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
    // assert_eq!(
    //     err,
    //     ContractError::InvalidPermission {
    //         msg: "Contract permissions aren't allowed in the address list contract".to_string()
    //     }
    // );

    // Test remove actor
    let msg = ExecuteMsg::RemovePermissions {
        actors: vec![AndrAddr::from_string(actor.clone())],
    };
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
    let permission = PERMISSIONS.may_load(deps.as_ref().storage, &actor).unwrap();
    assert!(permission.is_none());

    // Try with unauthorized address
    let unauth_info = mock_info("anyone", &[]);
    let res = execute(deps.as_mut(), env.clone(), unauth_info, msg).unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, res);

    // Try removing an actor that isn't included in permissions
    let random_actor = Addr::unchecked("random_actor");
    let msg = ExecuteMsg::RemovePermissions {
        actors: vec![AndrAddr::from_string(random_actor)],
    };
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::ActorNotFound {})
}

#[test]
fn test_add_remove_multiple_actors() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();

    let operator = "creator";
    let info = mock_info(operator, &[]);

    let actors = vec![
        AndrAddr::from_string("actor1"),
        AndrAddr::from_string("actor2"),
    ];
    let permission = LocalPermission::default();

    init(deps.as_mut(), info.clone());

    let msg = ExecuteMsg::PermissionActors {
        actors: actors.clone(),
        permission: permission.clone(),
    };

    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
    let expected = Response::default().add_attributes(vec![
        attr("action", "add_actor_permission"),
        attr("actor", "actor1, actor2"),
        attr("permission", permission.to_string()),
    ]);
    assert_eq!(expected, res);

    // Check that the actor and permission have been saved.
    let new_permission = PERMISSIONS
        .load(
            deps.as_ref().storage,
            &actors[0].get_raw_address(&deps.as_ref()).unwrap(),
        )
        .unwrap();
    assert_eq!(new_permission, permission);
    let new_permission = PERMISSIONS
        .load(
            deps.as_ref().storage,
            &actors[1].get_raw_address(&deps.as_ref()).unwrap(),
        )
        .unwrap();
    assert_eq!(new_permission, permission);

    // Try with unauthorized address
    let unauth_info = mock_info("anyone", &[]);
    let res = execute(deps.as_mut(), env.clone(), unauth_info, msg).unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, res);

    // // Contract permissions aren't allowed to be saved in the address list contract
    // let contract_permission = Permission::Whitelisted(None);
    // let msg = ExecuteMsg::PermissionActors {
    //     actor: Addr::unchecked(MOCK_KERNEL_CONTRACT),
    //     permission: contract_permission,
    // };
    // let err = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
    // assert_eq!(
    //     err,
    //     ContractError::InvalidPermission {
    //         msg: "Contract permissions aren't allowed in the address list contract".to_string()
    //     }
    // );

    // Test remove actor
    let msg = ExecuteMsg::RemovePermissions {
        actors: actors.clone(),
    };
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
    let permission = PERMISSIONS
        .may_load(
            deps.as_ref().storage,
            &actors[0].get_raw_address(&deps.as_ref()).unwrap(),
        )
        .unwrap();
    assert!(permission.is_none());
    let permission = PERMISSIONS
        .may_load(
            deps.as_ref().storage,
            &actors[1].get_raw_address(&deps.as_ref()).unwrap(),
        )
        .unwrap();
    assert!(permission.is_none());

    // Try with unauthorized address
    let unauth_info = mock_info("anyone", &[]);
    let res = execute(deps.as_mut(), env.clone(), unauth_info, msg).unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, res);

    // Try removing an actor that isn't included in permissions
    let random_actor = Addr::unchecked("random_actor");
    let msg = ExecuteMsg::RemovePermissions {
        actors: vec![AndrAddr::from_string(random_actor)],
    };
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(err, ContractError::ActorNotFound {})
}

#[test]
fn test_includes_actor_query() {
    let mut deps = mock_dependencies_custom(&[]);

    let actor = Addr::unchecked("actor");
    let random_actor = Addr::unchecked("random_actor");

    let permission = LocalPermission::default();

    PERMISSIONS
        .save(deps.as_mut().storage, &actor, &permission)
        .unwrap();

    let msg = QueryMsg::IncludesActor { actor };

    let res: IncludesActorResponse =
        from_json(query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

    assert_eq!(IncludesActorResponse { included: true }, res);

    let msg = QueryMsg::IncludesActor {
        actor: random_actor,
    };

    let res: IncludesActorResponse =
        from_json(query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

    assert_eq!(IncludesActorResponse { included: false }, res);
}

#[test]
fn test_actor_permission_query() {
    let mut deps = mock_dependencies_custom(&[]);

    let actor = Addr::unchecked("actor");
    let random_actor = Addr::unchecked("random_actor");

    let permission = LocalPermission::default();

    PERMISSIONS
        .save(deps.as_mut().storage, &actor, &permission)
        .unwrap();

    let msg = QueryMsg::ActorPermission { actor };

    let res: ActorPermissionResponse =
        from_json(query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

    assert_eq!(
        ActorPermissionResponse {
            permission: LocalPermission::default()
        },
        res
    );

    // Try querying for an actor that isn't in permissions
    let msg = QueryMsg::ActorPermission {
        actor: random_actor,
    };

    let err = query(deps.as_ref(), mock_env(), msg).unwrap_err();
    assert_eq!(err, ContractError::ActorNotFound {});
}
