use cosmwasm_std::{testing::{mock_dependencies, mock_env, mock_info}, Api, Coin, Uint128, Response, attr, SubMsg, CosmosMsg, WasmMsg, to_binary, coin};
use cw20::Cw20ExecuteMsg;
use andromeda_protocol::anchor::{AnchorMarketMsg, ExecuteMsg, InstantiateMsg, YourselfMsg};
use crate::contract::{execute, instantiate};
use crate::state::{CONFIG, POSITION};
use crate::testing::mock_querier::mock_dependencies_custom;


#[test]
fn test_instantiate() {
    let mut deps = mock_dependencies(&[]);
    let env = mock_env();
    let owner = "owner";
    let info = mock_info(owner, &[]);
    let msg = InstantiateMsg {
        anchor_token: "anchor_token".to_string(),
        anchor_mint: "anchor_mint".to_string(),
        stable_denom: "uusd".to_string()
    };
    let res = instantiate(deps.as_mut(), env, info, msg.clone()).unwrap();

    assert_eq!(0, res.messages.len());

    //checking
    let config = CONFIG.load(deps.as_ref().storage).unwrap();

    assert_eq!(msg.anchor_token, deps.api.addr_humanize(&config.anchor_token).unwrap().to_string());
    assert_eq!(msg.anchor_mint, deps.api.addr_humanize(&config.anchor_mint).unwrap().to_string());
    assert_eq!(msg.stable_denom, config.stable_denom);
}
#[test]
fn test_deposit(){
    let mut deps =mock_dependencies_custom(&[]);
    let msg =InstantiateMsg{
        anchor_token: "aust_token".to_string(),
        anchor_mint: "anchor_mint".to_string(),
        stable_denom: "uusd".to_string()
    };
    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg =ExecuteMsg::Deposit {};
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );
    let env = mock_env();
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    let expected_res = Response::new().add_submessage(
        SubMsg::reply_on_success(
            CosmosMsg::Wasm(
                WasmMsg::Execute{
                    contract_addr: "anchor_mint".to_string(),
                    msg: to_binary( &AnchorMarketMsg::DepositStable{}).unwrap(),
                    funds: vec![coin(1000000u128, "uusd")]
                }),
            1u64
        )
    ).add_attributes(vec![
        attr("action", "deposit"),
        attr("deposit_amount", "1000000")
    ]);
    assert_eq!(res,expected_res)
}

#[test]
fn test_withdraw(){
    let mut deps =mock_dependencies_custom(&[]);
    let msg =InstantiateMsg{
        anchor_token: "aust_token".to_string(),
        anchor_mint: "anchor_mint".to_string(),
        stable_denom: "uusd".to_string()
    };
    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg =ExecuteMsg::Deposit {};
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );
    let env = mock_env();
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    //set aust_amount to position manually
    let mut position = POSITION.load(&deps.storage, &1u128.to_be_bytes()).unwrap();
    position.aust_amount = Uint128::from(1000000u128);
    POSITION.save(deps.as_mut().storage, &1u128.to_be_bytes(), &position).unwrap();

    let msg = ExecuteMsg::Withdraw { position_idx: Uint128::from(1u128) };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env,info,msg).unwrap();
    
    let expected_res = Response::new()
        .add_messages(vec![
            CosmosMsg::Wasm(
                    WasmMsg::Execute{
                        contract_addr: "aust_token".to_string(),
                        msg: to_binary( &Cw20ExecuteMsg::Send {
                            contract: "anchor_mint".to_string(),
                            amount: Uint128::from(1000000u128),
                            msg: to_binary(&AnchorMarketMsg::RedeemStable {}).unwrap()
                        }).unwrap(),
                        funds: vec![]
                    }),
            CosmosMsg::Wasm(
                WasmMsg::Execute{
                    contract_addr: "cosmos2contract".to_string(),
                    msg: to_binary( &ExecuteMsg::Yourself{
                        yourself_msg: YourselfMsg::TransferUst { receiver: "addr0000".to_string() }
                    }).unwrap(),
                    funds: vec![]
                }),
        ]
    ).add_attributes(
        vec![
            attr("action","withdraw"),
            attr("position_idx", "1")
        ]
    );
    assert_eq!(res,expected_res)
}