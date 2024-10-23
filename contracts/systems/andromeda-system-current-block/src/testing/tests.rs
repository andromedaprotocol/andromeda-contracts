use crate::contract::{instantiate, query};
use andromeda_std::testing::mock_querier::{mock_dependencies_custom, MOCK_KERNEL_CONTRACT};
use andromeda_systems::current_block::{InstantiateMsg, QueryMsg};

use cosmwasm_std::{
    from_json,
    testing::{mock_env, mock_info},
};

#[test]
fn test_instantiation() {
    let mut deps = mock_dependencies_custom(&[]);

    let msg = InstantiateMsg {
        name: String::from("Current Block"),
        root: String::from("chain"),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };
    let info = mock_info("creator", &[]);

    // we can just call .unwrap() to assert this was a success
    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(1, res.messages.len());
}

#[test]
fn test_query_current_block_height() {
    let mut deps = mock_dependencies_custom(&[]);

    let msg = InstantiateMsg {
        name: String::from("Current Block"),
        root: String::from("chain"),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };
    let info = mock_info("creator", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let mut env = mock_env();
    env.block.height = 10000;

    let res = query(deps.as_ref(), env, QueryMsg::GetCurrentBlockHeight {}).unwrap();
    let current_block_height: String = from_json(res).unwrap();

    assert_eq!(current_block_height, 10000.to_string());
}
