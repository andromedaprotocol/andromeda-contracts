use crate::contract::instantiate;
use andromeda_protocol::splitter::{AddressPercent, InstantiateMsg};
use andromeda_protocol::{
    address_list::InstantiateMsg as AddressListInstantiateMsg,
    modules::address_list::{AddressListModule, REPLY_ADDRESS_LIST},
};
use common::ado_base::recipient::Recipient;
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{attr, to_binary, CosmosMsg, Decimal, ReplyOn, Response, SubMsg, WasmMsg};

#[test]
fn test_instantiate() {
    let mut deps = mock_dependencies(&[]);
    let env = mock_env();
    let info = mock_info("creator", &[]);
    let address_list = Some(AddressListModule {
        address: None,
        code_id: Some(1u64),
        operators: Some(vec!["creator".to_string()]),
        inclusive: true,
    });
    let msg = InstantiateMsg {
        address_list,
        recipients: vec![AddressPercent {
            recipient: Recipient::from_string(String::from("Some Address")),
            percent: Decimal::percent(100),
        }],
    };
    let res = instantiate(deps.as_mut(), env, info, msg).unwrap();
    let expected_res = Response::new()
        .add_attributes(vec![
            attr("method", "instantiate"),
            attr("type", "splitter"),
        ])
        .add_submessages(vec![SubMsg {
            id: REPLY_ADDRESS_LIST,
            gas_limit: None,
            reply_on: ReplyOn::Always,
            msg: CosmosMsg::Wasm(WasmMsg::Instantiate {
                admin: Some("creator".to_string()),
                code_id: 1u64,
                funds: vec![],
                label: String::from("Address list instantiation"),
                msg: to_binary(&AddressListInstantiateMsg {
                    operators: vec!["creator".to_string()],
                    is_inclusive: true,
                })
                .unwrap(),
            }),
        }]);
    assert_eq!(res, expected_res);
}
