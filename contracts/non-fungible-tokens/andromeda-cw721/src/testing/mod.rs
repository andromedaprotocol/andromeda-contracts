use cosmwasm_std::{
    attr, coins, from_binary,
    testing::{mock_env, mock_info},
    Addr, Coin, DepsMut, Env, Response, Uint128,
};

use andromeda_std::amp::addresses::AndrAddr;
use andromeda_std::error::ContractError;
use andromeda_std::{ado_base::modules::Module, testing::mock_querier::FAKE_VFS_PATH};

use crate::{contract::*, state::ANDR_MINTER};
use andromeda_non_fungible_tokens::cw721::{
    ExecuteMsg, InstantiateMsg, MintMsg, QueryMsg, TokenExtension, TransferAgreement,
};
use andromeda_std::testing::mock_querier::{
    mock_dependencies_custom, MOCK_ADDRESS_LIST_CONTRACT, MOCK_KERNEL_CONTRACT,
};
use cw721::{AllNftInfoResponse, OwnerOfResponse};

const MINTER: &str = "minter";
const SYMBOL: &str = "TT";
const NAME: &str = "TestToken";
const ADDRESS_LIST: &str = "addresslist";
// const RATES: &str = "rates";

fn init_setup(deps: DepsMut, env: Env, modules: Option<Vec<Module>>) {
    let info = mock_info(MINTER, &[]);
    let inst_msg = InstantiateMsg {
        name: NAME.to_string(),
        symbol: SYMBOL.to_string(),
        minter: AndrAddr::from_string(MINTER.to_string()),
        modules,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    instantiate(deps, env, info, inst_msg).unwrap();
}

fn mint_token(deps: DepsMut, env: Env, token_id: String, owner: String, extension: TokenExtension) {
    let info = mock_info(MINTER, &[]);
    let mint_msg = ExecuteMsg::Mint {
        token_id,
        owner,
        token_uri: None,
        extension,
    };
    execute(deps, env, info, mint_msg).unwrap();
}

#[test]
fn test_transfer_nft() {
    let token_id = String::from("testtoken");
    let creator = String::from("creator");
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    init_setup(deps.as_mut(), env.clone(), None);
    assert_eq!(
        MINTER.to_owned(),
        ANDR_MINTER.load(deps.as_ref().storage).unwrap()
    );
    mint_token(
        deps.as_mut(),
        env.clone(),
        token_id.clone(),
        creator.clone(),
        TokenExtension {
            publisher: creator.clone(),
        },
    );

    let transfer_msg = ExecuteMsg::TransferNft {
        recipient: Addr::unchecked("recipient").to_string(),
        token_id: token_id.clone(),
    };

    let unauth_info = mock_info("anyone", &[]);
    assert_eq!(
        execute(
            deps.as_mut(),
            env.clone(),
            unauth_info,
            transfer_msg.clone()
        )
        .unwrap_err(),
        ContractError::Unauthorized {}
    );

    let info = mock_info(creator.as_str(), &[]);
    assert!(execute(deps.as_mut(), env.clone(), info, transfer_msg).is_ok());

    let query_msg = QueryMsg::OwnerOf {
        token_id,
        include_expired: None,
    };
    let query_resp = query(deps.as_ref(), env, query_msg).unwrap();
    let resp: OwnerOfResponse = from_binary(&query_resp).unwrap();
    assert_eq!(resp.owner, String::from("recipient"))
}

#[test]
fn test_agreed_transfer_nft() {
    let token_id = String::from("testtoken");
    let creator = String::from("creator");
    let valid_info = mock_info(creator.as_str(), &[]);
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let agreed_amount = Coin {
        denom: "uluna".to_string(),
        amount: Uint128::from(100u64),
    };
    let purchaser = "purchaser";
    init_setup(deps.as_mut(), env.clone(), None);
    mint_token(
        deps.as_mut(),
        env.clone(),
        token_id.clone(),
        creator.clone(),
        TokenExtension { publisher: creator },
    );

    let transfer_agreement_msg = ExecuteMsg::TransferAgreement {
        token_id: token_id.clone(),
        agreement: Some(TransferAgreement {
            amount: agreed_amount.clone(),
            purchaser: purchaser.to_string(),
        }),
    };
    execute(
        deps.as_mut(),
        env.clone(),
        valid_info,
        transfer_agreement_msg,
    )
    .unwrap();

    let transfer_msg = ExecuteMsg::TransferNft {
        recipient: Addr::unchecked("recipient").to_string(),
        token_id: token_id.clone(),
    };

    let invalid_info = mock_info(purchaser, &[]);
    assert_eq!(
        execute(
            deps.as_mut(),
            env.clone(),
            invalid_info,
            transfer_msg.clone()
        )
        .unwrap_err(),
        ContractError::InsufficientFunds {}
    );

    let info = mock_info(purchaser, &[agreed_amount]);
    assert!(execute(deps.as_mut(), env.clone(), info, transfer_msg).is_ok());

    let query_msg = QueryMsg::OwnerOf {
        token_id,
        include_expired: None,
    };
    let query_resp = query(deps.as_ref(), env, query_msg).unwrap();
    let resp: OwnerOfResponse = from_binary(&query_resp).unwrap();
    assert_eq!(resp.owner, String::from("recipient"))
}

#[test]
fn test_agreed_transfer_nft_wildcard() {
    let token_id = String::from("testtoken");
    let creator = String::from("creator");
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let agreed_amount = Coin {
        denom: "uluna".to_string(),
        amount: Uint128::from(100u64),
    };
    let purchaser = "*";
    init_setup(deps.as_mut(), env.clone(), None);
    mint_token(
        deps.as_mut(),
        env.clone(),
        token_id.clone(),
        creator.clone(),
        TokenExtension {
            publisher: creator.clone(),
        },
    );

    // Update transfer agreement.
    let msg = ExecuteMsg::TransferAgreement {
        token_id: token_id.clone(),
        agreement: Some(TransferAgreement {
            amount: agreed_amount.clone(),
            purchaser: purchaser.to_string(),
        }),
    };
    let _res = execute(deps.as_mut(), mock_env(), mock_info(&creator, &[]), msg).unwrap();

    // Transfer the nft
    let transfer_msg = ExecuteMsg::TransferNft {
        recipient: Addr::unchecked("recipient").to_string(),
        token_id: token_id.clone(),
    };

    let info = mock_info("anyone", &[agreed_amount]);
    let _res = execute(deps.as_mut(), env.clone(), info, transfer_msg).unwrap();

    let query_msg = QueryMsg::OwnerOf {
        token_id,
        include_expired: None,
    };
    let query_resp = query(deps.as_ref(), env, query_msg).unwrap();
    let resp: OwnerOfResponse = from_binary(&query_resp).unwrap();
    assert_eq!(resp.owner, String::from("recipient"))
}

#[test]
fn test_archive() {
    let token_id = String::from("testtoken");
    let creator = String::from("creator");
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    init_setup(deps.as_mut(), env.clone(), None);
    mint_token(
        deps.as_mut(),
        env.clone(),
        token_id.clone(),
        creator.clone(),
        TokenExtension {
            publisher: creator.clone(),
        },
    );

    let msg = ExecuteMsg::Archive {
        token_id: token_id.clone(),
    };

    let unauth_info = mock_info("anyone", &[]);
    assert_eq!(
        execute(deps.as_mut(), env.clone(), unauth_info, msg.clone()).unwrap_err(),
        ContractError::Unauthorized {}
    );

    let info = mock_info(creator.as_str(), &[]);
    assert!(execute(deps.as_mut(), env.clone(), info, msg).is_ok());

    let query_msg = QueryMsg::IsArchived { token_id };
    let query_resp = query(deps.as_ref(), env, query_msg).unwrap();
    let resp: bool = from_binary(&query_resp).unwrap();
    assert!(resp)
}

#[test]
fn test_burn() {
    let token_id = String::from("testtoken");
    let creator = String::from("creator");
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    init_setup(deps.as_mut(), env.clone(), None);
    mint_token(
        deps.as_mut(),
        env.clone(),
        token_id.clone(),
        creator.clone(),
        TokenExtension {
            publisher: creator.clone(),
        },
    );

    let msg = ExecuteMsg::Burn {
        token_id: token_id.clone(),
    };

    let unauth_info = mock_info("anyone", &[]);
    assert_eq!(
        execute(deps.as_mut(), env.clone(), unauth_info, msg.clone()).unwrap_err(),
        ContractError::Unauthorized {}
    );

    let info = mock_info(creator.as_str(), &[]);
    let res = execute(deps.as_mut(), env, info.clone(), msg).unwrap();

    assert_eq!(
        Response::default().add_attributes(vec![
            attr("action", "burn"),
            attr("token_id", &token_id),
            attr("sender", info.sender.to_string()),
        ]),
        res
    );

    let contract = AndrCW721Contract::default();
    assert_eq!(
        None,
        contract
            .tokens
            .may_load(deps.as_ref().storage, &token_id)
            .unwrap()
    );

    assert_eq!(0, contract.token_count.load(deps.as_ref().storage).unwrap());
}

#[test]
fn test_archived_check() {
    let token_id = String::from("testtoken");
    let creator = String::from("creator");
    let valid_info = mock_info(creator.as_str(), &[]);
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    init_setup(deps.as_mut(), env.clone(), None);
    mint_token(
        deps.as_mut(),
        env.clone(),
        token_id.clone(),
        creator.clone(),
        TokenExtension {
            publisher: creator.clone(),
        },
    );

    let archive_msg = ExecuteMsg::Archive {
        token_id: token_id.clone(),
    };
    execute(deps.as_mut(), env.clone(), valid_info, archive_msg).unwrap();

    let msg = ExecuteMsg::Burn { token_id };

    let info = mock_info(creator.as_str(), &[]);
    assert_eq!(
        execute(deps.as_mut(), env, info, msg).unwrap_err(),
        ContractError::TokenIsArchived {}
    );
}

#[test]
fn test_transfer_agreement() {
    let token_id = String::from("testtoken");
    let creator = String::from("creator");
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let agreement = TransferAgreement {
        purchaser: String::from("purchaser"),
        amount: Coin {
            amount: Uint128::from(100u64),
            denom: "uluna".to_string(),
        },
    };
    init_setup(deps.as_mut(), env.clone(), None);
    mint_token(
        deps.as_mut(),
        env.clone(),
        token_id.clone(),
        creator.clone(),
        TokenExtension {
            publisher: creator.clone(),
        },
    );

    let msg = ExecuteMsg::TransferAgreement {
        token_id: token_id.clone(),
        agreement: Some(agreement.clone()),
    };

    let unauth_info = mock_info("anyone", &[]);
    assert_eq!(
        execute(deps.as_mut(), env.clone(), unauth_info, msg.clone()).unwrap_err(),
        ContractError::Unauthorized {}
    );

    let info = mock_info(creator.as_str(), &[]);
    assert!(execute(deps.as_mut(), env.clone(), info, msg).is_ok());

    let query_msg = QueryMsg::TransferAgreement { token_id };
    let query_resp = query(deps.as_ref(), env, query_msg).unwrap();
    let resp: Option<TransferAgreement> = from_binary(&query_resp).unwrap();
    assert!(resp.is_some());
    assert_eq!(resp, Some(agreement))
}

#[test]
fn test_modules() {
    let modules: Vec<Module> = vec![
        // Module::new(RATES, MOCK_RATES_CONTRACT, false),
        Module::new(ADDRESS_LIST, MOCK_ADDRESS_LIST_CONTRACT, false),
    ];

    let mut deps = mock_dependencies_custom(&coins(100, "uusd"));

    let token_id = String::from("testtoken");
    let creator = String::from("creator");
    let env = mock_env();
    let _agreement = TransferAgreement {
        purchaser: String::from("purchaser"),
        amount: Coin {
            amount: Uint128::from(100u64),
            denom: "uusd".to_string(),
        },
    };
    init_setup(deps.as_mut(), env.clone(), Some(modules));
    mint_token(
        deps.as_mut(),
        env,
        token_id,
        creator.clone(),
        TokenExtension { publisher: creator },
    );

    // let msg = ExecuteMsg::TransferAgreement {
    //     token_id: token_id.clone(),
    //     agreement: Some(agreement),
    // };

    // let not_whitelisted_info = mock_info("not_whitelisted", &[]);
    // let res = execute(deps.as_mut(), mock_env(), not_whitelisted_info, msg.clone());
    // assert_eq!(
    //     ContractError::Std(StdError::generic_err(
    //         "Querier contract error: InvalidAddress"
    //     )),
    //     res.unwrap_err()
    // );

    // let info = mock_info("creator", &[]);
    // let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // let msg = ExecuteMsg::TransferNft {
    //     token_id: token_id.clone(),
    //     recipient: "purchaser".into(),
    // };

    // // Tax not added by sender, remember that the contract holds 100 uusd which is enough to cover
    // // the taxes in this case.
    // let purchaser = mock_info("purchaser", &coins(100, "uusd"));
    // let res = execute(deps.as_mut(), mock_env(), purchaser, msg.clone());
    // assert_eq!(ContractError::InsufficientFunds {}, res.unwrap_err());

    // // Add 10 for tax.
    // let purchaser = mock_info("purchaser", &coins(100 + 10, "uusd"));
    // let res = execute(deps.as_mut(), mock_env(), purchaser, msg).unwrap();

    // let sub_msgs: Vec<SubMsg> = vec![
    //     // For royalty.
    //     bank_sub_msg(MOCK_RATES_RECIPIENT, vec![coin(10, "uusd")]),
    //     // For tax.
    //     bank_sub_msg(MOCK_RATES_RECIPIENT, vec![coin(10, "uusd")]),
    //     bank_sub_msg(&creator, vec![coin(80, "uusd")]),
    // ];

    // assert_eq!(
    //     Response::new()
    //         .add_attribute("action", "transfer")
    //         .add_attribute("recipient", "purchaser")
    //         .add_submessages(sub_msgs)
    //         .add_event(Event::new("Royalty"))
    //         .add_event(Event::new("Tax")),
    //     res
    // );

    // // Test the hook.
    // let msg = QueryMsg::AndrHook(AndromedaHook::OnFundsTransfer {
    //     sender: "sender".to_string(),
    //     payload: to_binary(&token_id).unwrap(),
    //     amount: Funds::Native(coin(100, "uusd")),
    // });

    // let res: OnFundsTransferResponse =
    //     from_binary(&query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

    // let expected_response = OnFundsTransferResponse {
    //     msgs: vec![
    //         bank_sub_msg(MOCK_RATES_RECIPIENT, vec![coin(10, "uusd")]),
    //         bank_sub_msg(MOCK_RATES_RECIPIENT, vec![coin(10, "uusd")]),
    //     ],
    //     leftover_funds: Funds::Native(coin(90, "uusd")),
    //     events: vec![Event::new("Royalty"), Event::new("Tax")],
    // };
    // assert_eq!(expected_response, res);
}

#[test]
fn test_transfer_with_offer() {
    todo!("Implement with cw721 bids module");
    // let modules: Vec<Module> = vec![Module {
    //     module_name: Some("bids".to_owned()),
    //     address: MOCK_BIDS_CONTRACT.to_owned(),
    //     is_mutable: false,
    // }];

    // let mut deps = mock_dependencies_custom(&coins(100, "uusd"));

    // let token_id = String::from("testtoken");
    // let creator = String::from("creator");
    // let env = mock_env();
    // init_setup(deps.as_mut(), env.clone(), Some(modules));
    // mint_token(
    //     deps.as_mut(),
    //     env,
    //     token_id.clone(),
    //     creator.clone(),
    //     TokenExtension {
    //         publisher: creator.clone(),
    //     },
    // );

    // let msg = ExecuteMsg::TransferNft {
    //     recipient: "purchaser".to_string(),
    //     token_id: token_id.clone(),
    // };
    // let info = mock_info(&creator, &[]);
    // let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    // let msg: SubMsg = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
    //     contract_addr: MOCK_BIDS_CONTRACT.to_owned(),
    //     funds: vec![],
    //     msg: to_binary(&BidsExecuteMsg::AcceptBid {
    //         token_id,
    //         recipient: creator,
    //     })
    //     .unwrap(),
    // }));
    // assert_eq!(
    //     Response::new()
    //         .add_submessage(msg)
    //         .add_attribute("action", "transfer")
    //         .add_attribute("recipient", "purchaser"),
    //     res
    // );
}

#[test]
fn test_update_app_contract() {
    let mut deps = mock_dependencies_custom(&[]);

    let info = mock_info("app_contract", &[]);
    let inst_msg = InstantiateMsg {
        name: NAME.to_string(),
        symbol: SYMBOL.to_string(),
        minter: AndrAddr::from_string("eee"),
        modules: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), inst_msg).unwrap();

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
fn test_update_app_contract_invalid_minter() {
    let mut deps = mock_dependencies_custom(&[]);

    let info = mock_info("app_contract", &[]);
    let inst_msg = InstantiateMsg {
        name: NAME.to_string(),
        symbol: SYMBOL.to_string(),
        minter: AndrAddr::from_string(FAKE_VFS_PATH),
        modules: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    instantiate(deps.as_mut(), mock_env(), info.clone(), inst_msg).unwrap();

    let msg = ExecuteMsg::Mint {
        token_id: "1".to_string(),
        owner: "owner".to_string(),
        token_uri: None,
        extension: TokenExtension {
            publisher: "publisher".to_string(),
        },
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);
    assert!(res.is_err());
}

#[test]
fn test_batch_mint() {
    let mut deps = mock_dependencies_custom(&[]);
    let info = mock_info(MINTER, &[]);
    let inst_msg = InstantiateMsg {
        name: NAME.to_string(),
        symbol: SYMBOL.to_string(),
        minter: AndrAddr::from_string(MINTER),
        modules: None,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };
    let owner = "owner";
    let mut mint_msgs: Vec<MintMsg> = Vec::new();

    let mut i: i32 = 0;
    while i < 5 {
        let mint_msg = MintMsg {
            token_id: i.to_string(),
            owner: owner.to_string(),
            token_uri: None,
            extension: TokenExtension {
                publisher: owner.to_string(),
            },
        };
        i += 1;
        mint_msgs.push(mint_msg)
    }

    instantiate(deps.as_mut(), mock_env(), info.clone(), inst_msg).unwrap();

    let msg: ExecuteMsg = ExecuteMsg::BatchMint { tokens: mint_msgs };

    let _resp = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let mut i: i32 = 0;
    while i < 5 {
        let query_msg = QueryMsg::AllNftInfo {
            token_id: i.to_string(),
            include_expired: None,
        };
        let query_resp = query(deps.as_ref(), mock_env(), query_msg).unwrap();
        let info: AllNftInfoResponse<TokenExtension> = from_binary(&query_resp).unwrap();
        assert_eq!(info.access.owner, owner.to_string());
        i += 1;
    }
}
