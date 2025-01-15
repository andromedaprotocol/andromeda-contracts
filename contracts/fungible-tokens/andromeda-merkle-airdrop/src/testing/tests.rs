use andromeda_fungible_tokens::airdrop::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, IsClaimedResponse, LatestStageResponse,
    MerkleRootResponse, QueryMsg, TotalClaimedResponse,
};
use andromeda_std::{
    ado_contract::ADOContract,
    amp::AndrAddr,
    common::{denom::Asset, expiration::Expiry, Milliseconds},
    error::ContractError,
    testing::mock_querier::{MOCK_CW20_CONTRACT, MOCK_KERNEL_CONTRACT},
};
use cosmwasm_schema::{cw_serde, serde::Deserialize};
use cosmwasm_std::{
    attr, coin, from_json,
    testing::{mock_env, mock_info},
    to_json_binary, BankMsg, Coin, CosmosMsg, SubMsg, Timestamp, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

use crate::{
    contract::{execute, instantiate, query},
    testing::mock_querier::mock_dependencies_custom,
};

const MOCK_NATIVE_DENOM: &str = "uandr";

#[test]
fn proper_instantiation() {
    let mut deps = mock_dependencies_custom(&[]);

    let msg = InstantiateMsg {
        asset_info: Asset::Cw20Token(AndrAddr::from_string(MOCK_CW20_CONTRACT)),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let env = mock_env();
    let info = mock_info("owner0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // it worked, let's query the state
    let res = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_json(res).unwrap();
    assert!(ADOContract::default()
        .is_contract_owner(deps.as_ref().storage, "owner0000")
        .unwrap());
    assert_eq!(
        Asset::Cw20Token(AndrAddr::from_string(MOCK_CW20_CONTRACT)),
        config.asset_info
    );

    let res = query(deps.as_ref(), env, QueryMsg::LatestStage {}).unwrap();
    let latest_stage: LatestStageResponse = from_json(res).unwrap();
    assert_eq!(0u8, latest_stage.latest_stage);
}

#[test]
fn register_merkle_root() {
    let mut deps = mock_dependencies_custom(&[]);

    let msg = InstantiateMsg {
        asset_info: Asset::Cw20Token(AndrAddr::from_string(MOCK_CW20_CONTRACT)),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let env = mock_env();
    let info = mock_info("owner0000", &[]);
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    // register new merkle root
    let env = mock_env();
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::RegisterMerkleRoot {
        merkle_root: "634de21cde1044f41d90373733b0f0fb1c1c71f9652b905cdf159e73c4cf0d37".to_string(),
        expiration: None,

        total_amount: None,
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "register_merkle_root"),
            attr("stage", "1"),
            attr(
                "merkle_root",
                "634de21cde1044f41d90373733b0f0fb1c1c71f9652b905cdf159e73c4cf0d37"
            ),
            attr("total_amount", "0")
        ]
    );

    let res = query(deps.as_ref(), env.clone(), QueryMsg::LatestStage {}).unwrap();
    let latest_stage: LatestStageResponse = from_json(res).unwrap();
    assert_eq!(1u8, latest_stage.latest_stage);

    let res = query(
        deps.as_ref(),
        env,
        QueryMsg::MerkleRoot {
            stage: latest_stage.latest_stage,
        },
    )
    .unwrap();
    let merkle_root: MerkleRootResponse = from_json(res).unwrap();
    assert_eq!(
        "634de21cde1044f41d90373733b0f0fb1c1c71f9652b905cdf159e73c4cf0d37".to_string(),
        merkle_root.merkle_root
    );
}

const TEST_DATA_1: &[u8] = include_bytes!("../testdata/airdrop_stage_1_test_data.json");
const TEST_DATA_2: &[u8] = include_bytes!("../testdata/airdrop_stage_2_test_data.json");

#[cw_serde]
struct Encoded {
    account: String,
    amount: Uint128,
    root: String,
    proofs: Vec<String>,
}

#[test]
fn test_claim() {
    // Run test 1
    let mut deps = mock_dependencies_custom(&[]);
    let test_data: Encoded = from_json(TEST_DATA_1).unwrap();

    let msg = InstantiateMsg {
        asset_info: Asset::Cw20Token(AndrAddr::from_string(MOCK_CW20_CONTRACT)),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let env = mock_env();
    let info = mock_info("owner0000", &[]);
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    let env = mock_env();
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::RegisterMerkleRoot {
        merkle_root: test_data.root,
        expiration: None,

        total_amount: None,
    };
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::Claim {
        amount: test_data.amount,
        stage: 1u8,
        proof: test_data.proofs,
    };

    let env = mock_env();
    let info = mock_info(test_data.account.as_str(), &[]);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
    let expected = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: MOCK_CW20_CONTRACT.to_string(),
        funds: vec![],
        msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
            recipient: test_data.account.clone(),
            amount: test_data.amount,
        })
        .unwrap(),
    }));
    assert_eq!(res.messages, vec![expected]);

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim"),
            attr("stage", "1"),
            attr("address", test_data.account.clone()),
            attr("amount", test_data.amount)
        ]
    );

    // Check total claimed on stage 1
    assert_eq!(
        from_json::<TotalClaimedResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::TotalClaimed { stage: 1 }
            )
            .unwrap()
        )
        .unwrap()
        .total_claimed,
        test_data.amount
    );

    // Check address is claimed
    assert!(
        from_json::<IsClaimedResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::IsClaimed {
                    stage: 1,
                    address: test_data.account
                }
            )
            .unwrap()
        )
        .unwrap()
        .is_claimed
    );

    // check error on double claim
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(res, ContractError::Claimed {});

    // Second test
    let test_data: Encoded = from_json(TEST_DATA_2).unwrap();

    // register new drop
    let env = mock_env();
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::RegisterMerkleRoot {
        merkle_root: test_data.root,
        expiration: None,

        total_amount: None,
    };
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    // Claim next airdrop
    let msg = ExecuteMsg::Claim {
        amount: test_data.amount,
        stage: 2u8,
        proof: test_data.proofs,
    };

    let env = mock_env();
    let info = mock_info(test_data.account.as_str(), &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    let expected: SubMsg<_> = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: MOCK_CW20_CONTRACT.to_string(),
        funds: vec![],
        msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
            recipient: test_data.account.clone(),
            amount: test_data.amount,
        })
        .unwrap(),
    }));
    assert_eq!(res.messages, vec![expected]);

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim"),
            attr("stage", "2"),
            attr("address", test_data.account),
            attr("amount", test_data.amount)
        ]
    );

    // Check total claimed on stage 2
    assert_eq!(
        from_json::<TotalClaimedResponse>(
            &query(deps.as_ref(), env, QueryMsg::TotalClaimed { stage: 2 }).unwrap()
        )
        .unwrap()
        .total_claimed,
        test_data.amount
    );
}

const TEST_DATA_1_MULTI: &[u8] = include_bytes!("../testdata/airdrop_stage_1_test_multi_data.json");

#[cw_serde]
struct Proof {
    account: String,
    amount: Uint128,
    proofs: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct MultipleData {
    total_claimed_amount: Uint128,
    root: String,
    accounts: Vec<Proof>,
}

#[test]
fn test_claim_native() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    let test_data: Encoded = from_json(TEST_DATA_1).unwrap();

    let msg = InstantiateMsg {
        asset_info: Asset::NativeToken(MOCK_NATIVE_DENOM.to_string()),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let env = mock_env();
    let info = mock_info("owner0000", &[]);
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    let env = mock_env();
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::RegisterMerkleRoot {
        merkle_root: test_data.root,
        expiration: None,
        total_amount: None,
    };
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::Claim {
        amount: test_data.amount,
        stage: 1u8,
        proof: test_data.proofs,
    };

    let env = mock_env();
    let info = mock_info(test_data.account.as_str(), &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    let expected = SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
        to_address: test_data.account.clone(),
        amount: vec![Coin {
            amount: test_data.amount,
            denom: MOCK_NATIVE_DENOM.to_string(),
        }],
    }));
    assert_eq!(res.messages, vec![expected]);

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim"),
            attr("stage", "1"),
            attr("address", test_data.account.clone()),
            attr("amount", test_data.amount)
        ]
    );

    // Check total claimed on stage 1
    assert_eq!(
        from_json::<TotalClaimedResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::TotalClaimed { stage: 1 }
            )
            .unwrap()
        )
        .unwrap()
        .total_claimed,
        test_data.amount
    );

    // Check address is claimed
    assert!(
        from_json::<IsClaimedResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::IsClaimed {
                    stage: 1,
                    address: test_data.account
                }
            )
            .unwrap()
        )
        .unwrap()
        .is_claimed
    );
}

#[test]
fn test_multiple_claim() {
    // Run test 1
    let mut deps = mock_dependencies_custom(&[]);
    let test_data: MultipleData = from_json(TEST_DATA_1_MULTI).unwrap();

    let msg = InstantiateMsg {
        asset_info: Asset::Cw20Token(AndrAddr::from_string(MOCK_CW20_CONTRACT)),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let env = mock_env();
    let info = mock_info("owner0000", &[]);
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    let env = mock_env();
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::RegisterMerkleRoot {
        merkle_root: test_data.root,
        expiration: None,
        total_amount: None,
    };
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    // Loop accounts and claim
    for account in test_data.accounts.iter() {
        let msg = ExecuteMsg::Claim {
            amount: account.amount,
            stage: 1u8,
            proof: account.proofs.clone(),
        };

        let env = mock_env();
        let info = mock_info(account.account.as_str(), &[]);
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        let expected = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CW20_CONTRACT.to_string(),
            funds: vec![],
            msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                recipient: account.account.clone(),
                amount: account.amount,
            })
            .unwrap(),
        }));
        assert_eq!(res.messages, vec![expected]);

        assert_eq!(
            res.attributes,
            vec![
                attr("action", "claim"),
                attr("stage", "1"),
                attr("address", account.account.clone()),
                attr("amount", account.amount)
            ]
        );
    }

    // Check total claimed on stage 1
    let env = mock_env();
    assert_eq!(
        from_json::<TotalClaimedResponse>(
            &query(deps.as_ref(), env, QueryMsg::TotalClaimed { stage: 1 }).unwrap()
        )
        .unwrap()
        .total_claimed,
        test_data.total_claimed_amount
    );
}

// Check expiration. Chain height in tests is 12345
#[test]
fn test_stage_expires() {
    let mut deps = mock_dependencies_custom(&[]);

    let msg = InstantiateMsg {
        asset_info: Asset::Cw20Token(AndrAddr::from_string(MOCK_CW20_CONTRACT)),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let env = mock_env();
    let info = mock_info("owner0000", &[]);
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    // can register merkle root
    let env = mock_env();
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::RegisterMerkleRoot {
        merkle_root: "5d4f48f147cb6cb742b376dce5626b2a036f69faec10cd73631c791780e150fc".to_string(),
        expiration: Some(Expiry::AtTime(Milliseconds::from_nanos(100_000_000))),
        total_amount: None,
    };
    execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // can't claim expired
    let msg = ExecuteMsg::Claim {
        amount: Uint128::new(5),
        stage: 1u8,
        proof: vec![],
    };

    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(
        res,
        ContractError::StageExpired {
            stage: 1,
            expiration: Milliseconds::from_nanos(100_000_000)
        }
    )
}

#[test]
fn test_cant_burn() {
    let mut deps = mock_dependencies_custom(&[]);

    let msg = InstantiateMsg {
        asset_info: Asset::Cw20Token(AndrAddr::from_string(MOCK_CW20_CONTRACT)),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let env = mock_env();
    let info = mock_info("owner0000", &[]);
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    // can register merkle root
    let mut env = mock_env();
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::RegisterMerkleRoot {
        merkle_root: "5d4f48f147cb6cb742b376dce5626b2a036f69faec10cd73631c791780e150fc".to_string(),
        expiration: Some(Expiry::AtTime(Milliseconds::from_nanos(100_000_000))),
        total_amount: Some(Uint128::new(100000)),
    };
    execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // Can't burn not expired stage
    let msg = ExecuteMsg::Burn { stage: 1u8 };
    env.block.time = Timestamp::from_nanos(10_000_000);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(
        res,
        ContractError::StageNotExpired {
            stage: 1,
            expiration: Milliseconds::from_nanos(100_000_000)
        }
    )
}

#[test]
fn test_can_burn() {
    let mut deps = mock_dependencies_custom(&[]);
    let test_data: Encoded = from_json(TEST_DATA_1).unwrap();

    let msg = InstantiateMsg {
        asset_info: Asset::Cw20Token(AndrAddr::from_string(MOCK_CW20_CONTRACT)),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let mut env = mock_env();
    let info = mock_info("owner0000", &[]);
    let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::RegisterMerkleRoot {
        merkle_root: test_data.root,
        expiration: Some(Expiry::AtTime(Milliseconds::from_nanos(100_000_000))),
        total_amount: Some(Uint128::new(10000)),
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Claim some tokens
    let msg = ExecuteMsg::Claim {
        amount: test_data.amount,
        stage: 1u8,
        proof: test_data.proofs,
    };
    env.block.time = Timestamp::from_nanos(100_000_000 - 1);
    let info = mock_info(test_data.account.as_str(), &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    let expected = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: MOCK_CW20_CONTRACT.to_string(),
        funds: vec![],
        msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
            recipient: test_data.account.clone(),
            amount: test_data.amount,
        })
        .unwrap(),
    }));
    assert_eq!(res.messages, vec![expected]);

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "claim"),
            attr("stage", "1"),
            attr("address", test_data.account.clone()),
            attr("amount", test_data.amount)
        ]
    );

    // makes the stage expire
    env.block.time = Timestamp::from_nanos(100_000_000 + 2);

    // Can burn after expired stage
    let msg = ExecuteMsg::Burn { stage: 1u8 };

    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    let expected = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: MOCK_CW20_CONTRACT.to_string(),
        funds: vec![],
        msg: to_json_binary(&Cw20ExecuteMsg::Burn {
            amount: Uint128::new(9900),
        })
        .unwrap(),
    }));
    assert_eq!(res.messages, vec![expected]);

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "burn"),
            attr("stage", "1"),
            attr("address", "owner0000"),
            attr("amount", Uint128::new(9900)),
        ]
    );
}

#[test]
fn test_can_burn_native() {
    let mut deps = mock_dependencies_custom(&[coin(100000, MOCK_NATIVE_DENOM)]);
    let test_data: Encoded = from_json(TEST_DATA_1).unwrap();

    let msg = InstantiateMsg {
        asset_info: Asset::NativeToken(MOCK_NATIVE_DENOM.to_string()),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };

    let mut env = mock_env();
    let info = mock_info("owner0000", &[]);
    let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::RegisterMerkleRoot {
        merkle_root: test_data.root,
        expiration: Some(Expiry::AtTime(Milliseconds::from_nanos(100_000_000))),
        total_amount: Some(Uint128::new(10000)),
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Claim some tokens
    let msg = ExecuteMsg::Claim {
        amount: test_data.amount,
        stage: 1u8,
        proof: test_data.proofs,
    };

    let info = mock_info(test_data.account.as_str(), &[]);
    env.block.time = Timestamp::from_nanos(100_000_000 - 1);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // makes the stage expire
    env.block.time = Timestamp::from_nanos(100_000_000 + 1);

    // Can burn after expired stage
    let msg = ExecuteMsg::Burn { stage: 1u8 };

    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    let expected = SubMsg::new(CosmosMsg::Bank(BankMsg::Burn {
        amount: vec![Coin {
            amount: Uint128::new(9900),
            denom: MOCK_NATIVE_DENOM.to_string(),
        }],
    }));
    assert_eq!(res.messages, vec![expected]);

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "burn"),
            attr("stage", "1"),
            attr("address", "owner0000"),
            attr("amount", Uint128::new(9900)),
        ]
    );
}
