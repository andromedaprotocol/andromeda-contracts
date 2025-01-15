use andromeda_app::app::AppComponent;
use andromeda_app_contract::mock::{mock_andromeda_app, mock_claim_ownership_msg, MockAppContract};
use andromeda_auction::mock::{
    mock_andromeda_auction, mock_auction_instantiate_msg, mock_place_bid, mock_start_auction,
    mock_update_auction, MockAuction,
};
use andromeda_cw20::mock::{mock_andromeda_cw20, mock_cw20_instantiate_msg, mock_minter, MockCW20};
use andromeda_cw721::mock::{mock_andromeda_cw721, mock_cw721_instantiate_msg, MockCW721};

use andromeda_finance::splitter::AddressPercent;
use andromeda_non_fungible_tokens::auction::{AuctionStateResponse, Cw20HookMsg};
use andromeda_rates::mock::mock_andromeda_rates;
use andromeda_splitter::mock::{
    mock_andromeda_splitter, mock_splitter_instantiate_msg, mock_splitter_send_msg, MockSplitter,
};
use andromeda_std::{
    ado_base::{
        permissioning::{LocalPermission, Permission},
        rates::{LocalRate, LocalRateType, LocalRateValue, PercentRate, Rate},
    },
    amp::{AndrAddr, Recipient},
    common::{
        denom::Asset,
        expiration::{Expiry, MILLISECONDS_TO_NANOSECONDS_RATIO},
        Milliseconds,
    },
    error::ContractError,
};
use andromeda_testing::{
    mock::mock_app, mock_builder::MockAndromedaBuilder, mock_contract::MockContract,
};
use cosmwasm_std::{coin, to_json_binary, Addr, BlockInfo, Decimal, Timestamp, Uint128};
use cw20::Cw20Coin;
use cw_multi_test::Executor;

#[test]
fn test_auction_app_modules() {
    let mut router = mock_app(None);
    let andr = MockAndromedaBuilder::new(&mut router, "admin")
        .with_wallets(vec![
            ("owner", vec![]),
            ("buyer_one", vec![coin(1000, "uandr")]),
            ("buyer_two", vec![coin(1000, "uandr")]),
            ("recipient_one", vec![]),
            ("recipient_two", vec![]),
        ])
        .with_contracts(vec![
            ("cw721", mock_andromeda_cw721()),
            ("auction", mock_andromeda_auction()),
            ("app-contract", mock_andromeda_app()),
            ("splitter", mock_andromeda_splitter()),
        ])
        .build(&mut router);
    let owner = andr.get_wallet("owner");
    let buyer_one = andr.get_wallet("buyer_one");
    let buyer_two = andr.get_wallet("buyer_two");
    let recipient_one = andr.get_wallet("recipient_one");
    let recipient_two = andr.get_wallet("recipient_two");

    // Generate App Components
    let cw721_init_msg = mock_cw721_instantiate_msg(
        "Test Tokens".to_string(),
        "TT".to_string(),
        owner.to_string(),
        andr.kernel.addr().to_string(),
        None,
    );
    let cw721_component = AppComponent::new(
        "cw721".to_string(),
        "cw721".to_string(),
        to_json_binary(&cw721_init_msg).unwrap(),
    );

    let auction_init_msg =
        mock_auction_instantiate_msg(andr.kernel.addr().to_string(), None, None, None);
    let auction_component = AppComponent::new(
        "auction".to_string(),
        "auction".to_string(),
        to_json_binary(&auction_init_msg).unwrap(),
    );

    let splitter_init_msg = mock_splitter_instantiate_msg(
        vec![
            AddressPercent {
                recipient: Recipient::new(recipient_one, None),
                percent: Decimal::percent(50),
            },
            AddressPercent {
                recipient: Recipient::new(recipient_two, None),
                percent: Decimal::percent(50),
            },
        ],
        andr.kernel.addr().to_string(),
        None,
        None,
        None,
    );
    let splitter_component = AppComponent::new(
        "splitter".to_string(),
        "splitter".to_string(),
        to_json_binary(&splitter_init_msg).unwrap(),
    );

    // Create App
    let app_components = vec![
        cw721_component.clone(),
        auction_component.clone(),
        splitter_component.clone(),
    ];
    let app = MockAppContract::instantiate(
        andr.get_code_id(&mut router, "app-contract"),
        owner,
        &mut router,
        "Auction App",
        app_components,
        andr.kernel.addr(),
        Some(owner.to_string()),
    );

    router
        .execute_contract(
            owner.clone(),
            Addr::unchecked(app.addr().clone()),
            &mock_claim_ownership_msg(None),
            &[],
        )
        .unwrap();

    // Mint Tokens
    let cw721: MockCW721 = app.query_ado_by_component_name(&router, cw721_component.name);
    cw721
        .execute_quick_mint(&mut router, owner.clone(), 1, owner.to_string())
        .unwrap();

    // Send Token to Auction
    let auction: MockAuction = app.query_ado_by_component_name(&router, auction_component.name);

    // Set up splitter to send funds to recipient one and two
    let splitter: MockSplitter = app.query_ado_by_component_name(&router, splitter_component.name);

    // Set rates to auction
    auction
        .execute_add_rate(
            &mut router,
            owner.clone(),
            "Claim".to_string(),
            Rate::Local(LocalRate {
                rate_type: LocalRateType::Deductive,
                recipient: Recipient::new(
                    splitter.addr(),
                    Some(to_json_binary(&mock_splitter_send_msg(None)).unwrap()),
                ),
                value: LocalRateValue::Percent(PercentRate {
                    percent: Decimal::percent(50),
                }),
                description: None,
            }),
        )
        .unwrap();

    let start_time = Milliseconds::from_nanos(router.block_info().time.nanos())
        .plus_milliseconds(Milliseconds(100));
    let receive_msg = mock_start_auction(
        Some(Expiry::AtTime(start_time)),
        Expiry::AtTime(start_time.plus_milliseconds(Milliseconds(1000))),
        None,
        Asset::NativeToken("uandr".to_string()),
        None,
        None,
        None,
        None,
    );
    cw721
        .execute_send_nft(
            &mut router,
            owner.clone(),
            auction.addr(),
            "0",
            &receive_msg,
        )
        .unwrap();

    router.set_block(BlockInfo {
        height: router.block_info().height,
        time: start_time.into(),
        chain_id: router.block_info().chain_id,
    });

    // Query Auction State
    let auction_ids: Vec<Uint128> =
        auction.query_auction_ids(&mut router, "0".to_string(), cw721.addr().to_string());

    assert_eq!(auction_ids.len(), 1);

    let auction_id = auction_ids.first().unwrap();
    let auction_state = auction.query_auction_state(&mut router, *auction_id);

    assert_eq!(auction_state.coin_denom, "uandr".to_string());
    assert_eq!(auction_state.owner, owner.to_string());

    // Place Bid One
    auction.execute_place_bid(
        &mut router,
        buyer_one.clone(),
        "0".to_string(),
        cw721.addr().to_string(),
        &[coin(50, "uandr")],
    );

    // Check Bid Status One
    let bids = auction.query_bids(&mut router, *auction_id);
    assert_eq!(bids.len(), 1);

    let bid = bids.first().unwrap();
    assert_eq!(bid.bidder, buyer_one.to_string());
    assert_eq!(bid.amount, Uint128::from(50u128));

    auction.execute_place_bid(
        &mut router,
        buyer_two.clone(),
        "0".to_string(),
        cw721.addr().to_string(),
        &[coin(100, "uandr")],
    );

    // Check Bid Status One
    let bids = auction.query_bids(&mut router, *auction_id);
    assert_eq!(bids.len(), 2);

    let bid_two = bids.get(1).unwrap();
    assert_eq!(bid_two.bidder, buyer_two.to_string());
    assert_eq!(bid_two.amount, Uint128::from(100u128));

    // End Auction
    router.set_block(BlockInfo {
        height: router.block_info().height,
        time: start_time.plus_milliseconds(Milliseconds(1000)).into(),
        chain_id: router.block_info().chain_id,
    });
    auction
        .execute_claim_auction(
            &mut router,
            buyer_two.clone(),
            "0".to_string(),
            cw721.addr().to_string(),
        )
        .unwrap();

    // Check Final State
    let token_owner = cw721.query_owner_of(&router, "0");
    assert_eq!(token_owner, buyer_two);
    let owner_balance = router.wrap().query_balance(owner, "uandr").unwrap();
    assert_eq!(owner_balance.amount, Uint128::from(50u128));
    let recipient_one_balance = router.wrap().query_balance(recipient_one, "uandr").unwrap();
    assert_eq!(recipient_one_balance.amount, Uint128::from(25u128));
    let recipient_two_balance = router.wrap().query_balance(recipient_two, "uandr").unwrap();
    assert_eq!(recipient_two_balance.amount, Uint128::from(25u128));
}

#[test]
fn test_auction_app_recipient() {
    let mut router = mock_app(None);
    let andr = MockAndromedaBuilder::new(&mut router, "admin")
        .with_wallets(vec![
            ("owner", vec![]),
            ("buyer_one", vec![coin(1000, "uandr")]),
            ("buyer_two", vec![coin(1000, "uandr")]),
            ("recipient_one", vec![]),
            ("recipient_two", vec![]),
        ])
        .with_contracts(vec![
            ("cw721", mock_andromeda_cw721()),
            ("auction", mock_andromeda_auction()),
            ("app-contract", mock_andromeda_app()),
            ("splitter", mock_andromeda_splitter()),
        ])
        .build(&mut router);
    let owner = andr.get_wallet("owner");
    let buyer_one = andr.get_wallet("buyer_one");
    let buyer_two = andr.get_wallet("buyer_two");
    let recipient_one = andr.get_wallet("recipient_one");
    let recipient_two = andr.get_wallet("recipient_two");

    // Generate App Components
    let cw721_init_msg = mock_cw721_instantiate_msg(
        "Test Tokens".to_string(),
        "TT".to_string(),
        owner.to_string(),
        andr.kernel.addr().to_string(),
        None,
    );
    let cw721_component = AppComponent::new(
        "cw721".to_string(),
        "cw721".to_string(),
        to_json_binary(&cw721_init_msg).unwrap(),
    );

    let splitter_init_msg = mock_splitter_instantiate_msg(
        vec![
            AddressPercent::new(
                Recipient::from_string(format!("{recipient_one}")),
                Decimal::from_ratio(1u8, 2u8),
            ),
            AddressPercent::new(
                Recipient::from_string(format!("{recipient_two}")),
                Decimal::from_ratio(1u8, 2u8),
            ),
        ],
        andr.kernel.addr(),
        None,
        None,
        None,
    );
    let splitter_component = AppComponent::new(
        "splitter",
        "splitter",
        to_json_binary(&splitter_init_msg).unwrap(),
    );

    let auction_init_msg =
        mock_auction_instantiate_msg(andr.kernel.addr().to_string(), None, None, None);
    let auction_component = AppComponent::new(
        "auction".to_string(),
        "auction".to_string(),
        to_json_binary(&auction_init_msg).unwrap(),
    );

    // Create App
    let app_components = vec![
        cw721_component.clone(),
        auction_component.clone(),
        splitter_component,
    ];
    let app = MockAppContract::instantiate(
        andr.get_code_id(&mut router, "app-contract"),
        owner,
        &mut router,
        "Auction App",
        app_components,
        andr.kernel.addr(),
        Some(owner.to_string()),
    );

    router
        .execute_contract(
            owner.clone(),
            Addr::unchecked(app.addr().clone()),
            &mock_claim_ownership_msg(None),
            &[],
        )
        .unwrap();

    // Mint Tokens
    let cw721: MockCW721 = app.query_ado_by_component_name(&router, cw721_component.name);
    cw721
        .execute_quick_mint(&mut router, owner.clone(), 1, owner.to_string())
        .unwrap();

    // Send Token to Auction
    let auction: MockAuction = app.query_ado_by_component_name(&router, auction_component.name);
    let start_time = Milliseconds::from_nanos(router.block_info().time.nanos())
        .plus_milliseconds(Milliseconds(100));
    let receive_msg = mock_start_auction(
        Some(Expiry::AtTime(start_time)),
        Expiry::AtTime(start_time.plus_milliseconds(Milliseconds(1000))),
        None,
        Asset::NativeToken("uandr".to_string()),
        None,
        None,
        None,
        Some(Recipient::from_string("./splitter").with_msg(mock_splitter_send_msg(None))),
    );
    cw721
        .execute_send_nft(
            &mut router,
            owner.clone(),
            auction.addr(),
            "0",
            &receive_msg,
        )
        .unwrap();

    router.set_block(BlockInfo {
        height: router.block_info().height,
        time: start_time.into(),
        chain_id: router.block_info().chain_id,
    });

    // Query Auction State
    let auction_ids: Vec<Uint128> =
        auction.query_auction_ids(&mut router, "0".to_string(), cw721.addr().to_string());

    assert_eq!(auction_ids.len(), 1);

    let auction_id = auction_ids.first().unwrap();
    let auction_state = auction.query_auction_state(&mut router, *auction_id);

    assert_eq!(auction_state.coin_denom, "uandr".to_string());
    assert_eq!(auction_state.owner, owner.to_string());

    // Place Bid One
    auction.execute_place_bid(
        &mut router,
        buyer_one.clone(),
        "0".to_string(),
        cw721.addr().to_string(),
        &[coin(50, "uandr")],
    );

    // Check Bid Status One
    let bids = auction.query_bids(&mut router, *auction_id);
    assert_eq!(bids.len(), 1);

    let bid = bids.first().unwrap();
    assert_eq!(bid.bidder, buyer_one.to_string());
    assert_eq!(bid.amount, Uint128::from(50u128));

    auction.execute_place_bid(
        &mut router,
        buyer_two.clone(),
        "0".to_string(),
        cw721.addr().to_string(),
        &[coin(100, "uandr")],
    );

    // Check Bid Status One
    let bids = auction.query_bids(&mut router, *auction_id);
    assert_eq!(bids.len(), 2);

    let bid_two = bids.get(1).unwrap();
    assert_eq!(bid_two.bidder, buyer_two.to_string());
    assert_eq!(bid_two.amount, Uint128::from(100u128));

    // End Auction
    router.set_block(BlockInfo {
        height: router.block_info().height,
        time: start_time.plus_milliseconds(Milliseconds(1000)).into(),
        chain_id: router.block_info().chain_id,
    });

    auction
        .execute_claim_auction(
            &mut router,
            buyer_two.clone(),
            "0".to_string(),
            cw721.addr().to_string(),
        )
        .unwrap();

    // Check Final State
    let token_owner = cw721.query_owner_of(&router, "0");
    assert_eq!(token_owner, buyer_two);
    let owner_balance = router.wrap().query_balance(owner, "uandr").unwrap();
    assert_eq!(owner_balance.amount, Uint128::zero());
    let recipient_one_balance = router.wrap().query_balance(recipient_one, "uandr").unwrap();
    assert_eq!(recipient_one_balance.amount, Uint128::from(50u128));
    let recipient_two_balance = router.wrap().query_balance(recipient_two, "uandr").unwrap();
    assert_eq!(recipient_two_balance.amount, Uint128::from(50u128));
}

#[test]
fn test_auction_app_cw20_restricted() {
    let mut router = mock_app(None);
    let andr = MockAndromedaBuilder::new(&mut router, "admin")
        .with_wallets(vec![
            ("owner", vec![]),
            ("buyer_one", vec![coin(1000, "uandr")]),
            ("buyer_two", vec![coin(1000, "uandr")]),
            ("buyer_three", vec![coin(1000, "uandr")]),
            ("recipient_one", vec![]),
            ("recipient_two", vec![]),
        ])
        .with_contracts(vec![
            ("cw721", mock_andromeda_cw721()),
            ("cw20", mock_andromeda_cw20()),
            ("auction", mock_andromeda_auction()),
            ("app-contract", mock_andromeda_app()),
            ("rates", mock_andromeda_rates()),
            ("splitter", mock_andromeda_splitter()),
        ])
        .build(&mut router);
    let owner = andr.get_wallet("owner");
    let buyer_one = andr.get_wallet("buyer_one");
    let buyer_two = andr.get_wallet("buyer_two");
    let buyer_three = andr.get_wallet("buyer_three");
    // Generate App Components
    let cw721_init_msg = mock_cw721_instantiate_msg(
        "Test Tokens".to_string(),
        "TT".to_string(),
        owner.to_string(),
        andr.kernel.addr().to_string(),
        None,
    );
    let cw721_component = AppComponent::new(
        "cw721".to_string(),
        "cw721".to_string(),
        to_json_binary(&cw721_init_msg).unwrap(),
    );

    let buyer_one_original_balance = Uint128::new(1_000);
    let buyer_two_original_balance = Uint128::new(2_000);
    let owner_original_balance = Uint128::new(10_000);
    let initial_balances = vec![
        Cw20Coin {
            address: buyer_one.to_string(),
            amount: buyer_one_original_balance,
        },
        Cw20Coin {
            address: buyer_two.to_string(),
            amount: buyer_two_original_balance,
        },
        Cw20Coin {
            address: owner.to_string(),
            amount: owner_original_balance,
        },
    ];

    let cw20_init_msg = mock_cw20_instantiate_msg(
        None,
        "Test Tokens".to_string(),
        "TTT".to_string(),
        6,
        initial_balances.clone(),
        Some(mock_minter(
            owner.to_string(),
            Some(Uint128::from(1000000u128)),
        )),
        andr.kernel.addr().to_string(),
    );
    let cw20_component = AppComponent::new(
        "cw20".to_string(),
        "cw20".to_string(),
        to_json_binary(&cw20_init_msg).unwrap(),
    );

    let second_cw20_init_msg = mock_cw20_instantiate_msg(
        None,
        "Second Test Tokens".to_string(),
        "STTT".to_string(),
        6,
        initial_balances,
        Some(mock_minter(
            owner.to_string(),
            Some(Uint128::from(1000000u128)),
        )),
        andr.kernel.addr().to_string(),
    );
    let second_cw20_component = AppComponent::new(
        "second-cw20".to_string(),
        "cw20".to_string(),
        to_json_binary(&second_cw20_init_msg).unwrap(),
    );

    let auction_init_msg = mock_auction_instantiate_msg(
        andr.kernel.addr().to_string(),
        None,
        Some(vec![AndrAddr::from_string(format!(
            "./{}",
            cw721_component.name
        ))]),
        Some(vec![AndrAddr::from_string(format!(
            "./{}",
            cw20_component.name
        ))]),
    );
    let auction_component = AppComponent::new(
        "auction".to_string(),
        "auction".to_string(),
        to_json_binary(&auction_init_msg).unwrap(),
    );

    // Create App
    let app_components = vec![
        auction_component.clone(),
        cw721_component.clone(),
        cw20_component.clone(),
        second_cw20_component.clone(),
    ];

    let app = MockAppContract::instantiate(
        andr.get_code_id(&mut router, "app-contract"),
        owner,
        &mut router,
        "Auction App",
        app_components.clone(),
        andr.kernel.addr(),
        Some(owner.to_string()),
    );
    let components = app.query_components(&router);
    assert_eq!(components, app_components);
    let cw20: MockCW20 = app.query_ado_by_component_name(&router, cw20_component.name);

    // Mint Tokens
    let cw721: MockCW721 = app.query_ado_by_component_name(&router, cw721_component.name);
    cw721
        .execute_quick_mint(&mut router, owner.clone(), 2, owner.to_string())
        .unwrap();

    // Authorize NFT contract
    let auction: MockAuction = app.query_ado_by_component_name(&router, auction_component.name);
    auction
        .execute_authorize_token_address(&mut router, owner.clone(), cw721.addr(), None)
        .unwrap();

    // Send Token to Auction
    let start_time = router.block_info().time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO + 100;
    cw721
        .execute_send_nft(
            &mut router,
            owner.clone(),
            AndrAddr::from_string("./auction".to_string()),
            "0",
            &mock_start_auction(
                Some(Expiry::AtTime(Milliseconds(start_time))),
                Expiry::AtTime(Milliseconds(start_time + 2)),
                None,
                Asset::Cw20Token(AndrAddr::from_string(cw20.addr().to_string())),
                None,
                None,
                None,
                None,
            ),
        )
        .unwrap();

    router.set_block(BlockInfo {
        height: router.block_info().height,
        time: Timestamp::from_nanos(start_time * MILLISECONDS_TO_NANOSECONDS_RATIO),
        chain_id: router.block_info().chain_id,
    });

    // Query Auction State
    let auction_ids =
        auction.query_auction_ids(&mut router, "0".to_string(), cw721.addr().to_string());
    assert_eq!(auction_ids.len(), 1);

    let auction_id = auction_ids.first().unwrap();
    let auction_state: AuctionStateResponse = auction.query_auction_state(&mut router, *auction_id);
    assert_eq!(auction_state.coin_denom, cw20.addr().to_string());

    // Try to set permission with an empty vector of actors
    let actors = vec![];
    let action = "PlaceBid".to_string();
    let permission = Permission::Local(LocalPermission::blacklisted(None, None));
    let err: ContractError = auction
        .execute_set_permission(&mut router, owner.clone(), actors, action, permission)
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::NoActorsProvided {});

    // Place Bid One
    // Blacklist bidder now and blacklist bidder three just to test permissioning multiple actors at the same time
    let actors = vec![
        AndrAddr::from_string(buyer_one.clone()),
        AndrAddr::from_string(buyer_three.clone()),
    ];
    let action = "PlaceBid".to_string();
    let permission = Permission::Local(LocalPermission::blacklisted(None, None));
    auction
        .execute_set_permission(&mut router, owner.clone(), actors, action, permission)
        .unwrap();

    let bid_msg = mock_place_bid("0".to_string(), cw721.addr().to_string());

    // Bid should be rejected because we blacklisted bidder one
    let err: ContractError = router
        .execute_contract(
            buyer_one.clone(),
            Addr::unchecked(auction.addr().clone()),
            &bid_msg,
            &[coin(50, "uandr")],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Unauthorized {});

    // Bid should be rejected because we blacklisted bidder three
    let err: ContractError = router
        .execute_contract(
            buyer_three.clone(),
            Addr::unchecked(auction.addr().clone()),
            &bid_msg,
            &[coin(50, "uandr")],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Unauthorized {});

    // Now whitelist bidder one
    let actors = vec![AndrAddr::from_string(buyer_one.clone())];
    let action = "PlaceBid".to_string();
    let permission = Permission::Local(LocalPermission::whitelisted(None, None));
    auction
        .execute_set_permission(&mut router, owner.clone(), actors, action, permission)
        .unwrap();

    // Try bidding again
    let hook_msg = Cw20HookMsg::PlaceBid {
        token_id: "0".to_owned(),
        token_address: cw721.addr().clone().to_string(),
    };
    cw20.execute_send(
        &mut router,
        buyer_one.clone(),
        auction.addr(),
        Uint128::new(50),
        &hook_msg,
    )
    .unwrap();

    // Check Bid Status One
    let bids_resp = auction.query_bids(&mut router, *auction_id);
    assert_eq!(bids_resp.len(), 1);

    let bid = bids_resp.first().unwrap();
    assert_eq!(bid.bidder, buyer_one.to_string());
    assert_eq!(bid.amount, Uint128::from(50u128));

    // Second bid by buyer_two
    cw20.execute_send(
        &mut router,
        buyer_two.clone(),
        auction.addr(),
        Uint128::new(100),
        &hook_msg,
    )
    .unwrap();

    // Check Bid Status One
    let bids_resp = auction.query_bids(&mut router, *auction_id);
    assert_eq!(bids_resp.len(), 2);

    let bid_two = bids_resp.get(1).unwrap();
    assert_eq!(bid_two.bidder, buyer_two.to_string());
    assert_eq!(bid_two.amount, Uint128::from(100u128));

    // Forward time
    router.set_block(BlockInfo {
        height: router.block_info().height,
        time: Timestamp::from_nanos((start_time + 1001) * MILLISECONDS_TO_NANOSECONDS_RATIO),
        chain_id: router.block_info().chain_id,
    });

    // End Auction
    auction
        .execute_claim_auction(
            &mut router,
            buyer_two.clone(),
            "0".to_string(),
            cw721.addr().to_string(),
        )
        .unwrap();

    // Check Final State
    let owner_resp = cw721.query_owner_of(&router, "0".to_string());
    assert_eq!(owner_resp, buyer_two.to_string());

    // The auction's owner sold the NFT for 100, so the balance should increase by 100
    let cw20_balance = cw20.query_balance(&router, owner);
    assert_eq!(
        cw20_balance,
        owner_original_balance
            .checked_add(Uint128::new(100))
            .unwrap()
    );

    // Buyer two won the auction with a bid of 100, the balance should be 100 less than the original balance
    let cw20_balance = cw20.query_balance(&router, buyer_two);
    assert_eq!(
        cw20_balance,
        buyer_two_original_balance
            .checked_sub(Uint128::new(100))
            .unwrap()
    );

    // Buyer one was outbid, so the balance should remain unchanged
    let cw20_balance = cw20.query_balance(&router, buyer_one);
    assert_eq!(cw20_balance, buyer_one_original_balance);

    // Now try holding an auction with a recipient

    // Send Token to Auction
    let start_time = router.block_info().time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO + 100;
    cw721
        .execute_send_nft(
            &mut router,
            owner.clone(),
            AndrAddr::from_string("./auction".to_string()),
            "1",
            &mock_start_auction(
                Some(Expiry::AtTime(Milliseconds(start_time))),
                Expiry::AtTime(Milliseconds(start_time + 2)),
                None,
                Asset::Cw20Token(AndrAddr::from_string(cw20.addr().to_string())),
                None,
                None,
                Some(vec![buyer_one.clone(), buyer_two.clone()]),
                Some(Recipient::from_string(buyer_one)),
            ),
        )
        .unwrap();

    // Try updating denom to another unpermissioned cw20, shouldn't work since this a restricted cw20 auction
    let second_cw20: MockCW20 =
        app.query_ado_by_component_name(&router, second_cw20_component.name);
    let update_auction_msg = mock_update_auction(
        "0".to_string(),
        cw721.addr().to_string(),
        Some(Expiry::AtTime(Milliseconds(start_time))),
        Expiry::AtTime(Milliseconds(start_time + 2)),
        // This cw20 hasn't been permissioned
        Asset::Cw20Token(AndrAddr::from_string(second_cw20.addr().to_string())),
        None,
        None,
        None,
        Some(vec![buyer_one.clone(), buyer_two.clone()]),
        Some(Recipient::from_string(buyer_one)),
    );

    let err: ContractError = router
        .execute_contract(
            owner.clone(),
            auction.addr().clone(),
            &update_auction_msg,
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::InvalidFunds {
            msg: format!(
                "Non-permissioned CW20 asset '{}' set as denom.",
                second_cw20.addr()
            )
        }
    );

    router.set_block(BlockInfo {
        height: router.block_info().height,
        time: Timestamp::from_nanos(start_time * MILLISECONDS_TO_NANOSECONDS_RATIO),
        chain_id: router.block_info().chain_id,
    });

    // Query Auction State
    let auction_ids =
        auction.query_auction_ids(&mut router, "1".to_string(), cw721.addr().to_string());
    assert_eq!(auction_ids.len(), 1);

    let auction_id = auction_ids.first().unwrap();
    let auction_state: AuctionStateResponse = auction.query_auction_state(&mut router, *auction_id);
    assert_eq!(auction_state.coin_denom, cw20.addr().to_string());

    // Place Bid One
    // Whitelisted buyer one at the start of the auction
    let hook_msg = Cw20HookMsg::PlaceBid {
        token_id: "1".to_owned(),
        token_address: cw721.addr().clone().to_string(),
    };

    cw20.execute_send(
        &mut router,
        buyer_one.clone(),
        auction.addr(),
        Uint128::new(50),
        &hook_msg,
    )
    .unwrap();

    // Check Bid Status One
    let bids_resp = auction.query_bids(&mut router, *auction_id);
    assert_eq!(bids_resp.len(), 1);

    let bid = bids_resp.first().unwrap();
    assert_eq!(bid.bidder, buyer_one.to_string());
    assert_eq!(bid.amount, Uint128::from(50u128));

    // Second bid by buyer_two
    cw20.execute_send(
        &mut router,
        buyer_two.clone(),
        auction.addr(),
        Uint128::new(100),
        &hook_msg,
    )
    .unwrap();

    // Check Bid Status One
    let bids_resp = auction.query_bids(&mut router, *auction_id);
    assert_eq!(bids_resp.len(), 2);

    let bid_two = bids_resp.get(1).unwrap();
    assert_eq!(bid_two.bidder, buyer_two.to_string());
    assert_eq!(bid_two.amount, Uint128::from(100u128));

    // Forward time
    router.set_block(BlockInfo {
        height: router.block_info().height,
        time: Timestamp::from_nanos((start_time + 1001) * MILLISECONDS_TO_NANOSECONDS_RATIO),
        chain_id: router.block_info().chain_id,
    });

    // End Auction
    auction
        .execute_claim_auction(
            &mut router,
            buyer_two.clone(),
            "1".to_string(),
            cw721.addr().to_string(),
        )
        .unwrap();

    // Check Final State
    let owner_resp = cw721.query_owner_of(&router, "1".to_string());
    assert_eq!(owner_resp, buyer_two.to_string());

    // The auction's owner sold the NFT for 100, but has buyer_one set as recipient. So the balance shouldn't change since the previous auction
    let cw20_balance = cw20.query_balance(&router, owner);
    assert_eq!(
        cw20_balance,
        owner_original_balance
            .checked_add(Uint128::new(100))
            .unwrap()
    );

    // Buyer two won the auction with a bid of 100, the balance should be 100 less than the original balance
    let cw20_balance = cw20.query_balance(&router, buyer_two);
    assert_eq!(
        cw20_balance,
        buyer_two_original_balance
            // Purchase from previous and current auction
            .checked_sub(Uint128::new(100 + 100))
            .unwrap()
    );

    // Buyer one was outbid, but is set as the auction's recipient, so balance should increase by 100
    let cw20_balance = cw20.query_balance(&router, buyer_one);
    assert_eq!(
        cw20_balance,
        buyer_one_original_balance
            .checked_add(Uint128::new(100))
            .unwrap()
    );
}

#[test]
fn test_auction_app_cw20_unrestricted() {
    let mut router = mock_app(None);
    let andr = MockAndromedaBuilder::new(&mut router, "admin")
        .with_wallets(vec![
            ("owner", vec![]),
            ("buyer_one", vec![coin(1000, "uandr")]),
            ("buyer_two", vec![coin(1000, "uandr")]),
        ])
        .with_contracts(vec![
            ("cw721", mock_andromeda_cw721()),
            ("cw20", mock_andromeda_cw20()),
            ("auction", mock_andromeda_auction()),
            ("app-contract", mock_andromeda_app()),
            ("rates", mock_andromeda_rates()),
        ])
        .build(&mut router);
    let owner = andr.get_wallet("owner");
    let buyer_one = andr.get_wallet("buyer_one");
    let buyer_two = andr.get_wallet("buyer_two");

    // Generate App Components
    let cw721_init_msg = mock_cw721_instantiate_msg(
        "Test Tokens".to_string(),
        "TT".to_string(),
        owner.to_string(),
        andr.kernel.addr().to_string(),
        None,
    );
    let cw721_component = AppComponent::new(
        "cw721".to_string(),
        "cw721".to_string(),
        to_json_binary(&cw721_init_msg).unwrap(),
    );

    let buyer_one_original_balance = Uint128::new(1_000);
    let buyer_two_original_balance = Uint128::new(2_000);
    let owner_original_balance = Uint128::new(10_000);
    let initial_balances = vec![
        Cw20Coin {
            address: buyer_one.to_string(),
            amount: buyer_one_original_balance,
        },
        Cw20Coin {
            address: buyer_two.to_string(),
            amount: buyer_two_original_balance,
        },
        Cw20Coin {
            address: owner.to_string(),
            amount: owner_original_balance,
        },
    ];

    let cw20_init_msg = mock_cw20_instantiate_msg(
        None,
        "Test Tokens".to_string(),
        "TTT".to_string(),
        6,
        initial_balances.clone(),
        Some(mock_minter(
            owner.to_string(),
            Some(Uint128::from(1000000u128)),
        )),
        andr.kernel.addr().to_string(),
    );
    let cw20_component = AppComponent::new(
        "cw20".to_string(),
        "cw20".to_string(),
        to_json_binary(&cw20_init_msg).unwrap(),
    );

    let second_cw20_init_msg = mock_cw20_instantiate_msg(
        None,
        "Second Test Tokens".to_string(),
        "STTT".to_string(),
        6,
        initial_balances,
        Some(mock_minter(
            owner.to_string(),
            Some(Uint128::from(1000000u128)),
        )),
        andr.kernel.addr().to_string(),
    );
    let second_cw20_component = AppComponent::new(
        "second-cw20".to_string(),
        "cw20".to_string(),
        to_json_binary(&second_cw20_init_msg).unwrap(),
    );

    let auction_init_msg = mock_auction_instantiate_msg(
        andr.kernel.addr().to_string(),
        None,
        Some(vec![AndrAddr::from_string(format!(
            "./{}",
            cw721_component.name
        ))]),
        None,
    );
    let auction_component = AppComponent::new(
        "auction".to_string(),
        "auction".to_string(),
        to_json_binary(&auction_init_msg).unwrap(),
    );

    // Create App
    let app_components = vec![
        auction_component.clone(),
        cw721_component.clone(),
        cw20_component.clone(),
        second_cw20_component.clone(),
    ];

    let app = MockAppContract::instantiate(
        andr.get_code_id(&mut router, "app-contract"),
        owner,
        &mut router,
        "Auction App",
        app_components.clone(),
        andr.kernel.addr(),
        Some(owner.to_string()),
    );
    let components = app.query_components(&router);
    assert_eq!(components, app_components);
    let cw20: MockCW20 = app.query_ado_by_component_name(&router, cw20_component.name);

    // Mint Tokens
    let cw721: MockCW721 = app.query_ado_by_component_name(&router, cw721_component.name);
    cw721
        .execute_quick_mint(&mut router, owner.clone(), 2, owner.to_string())
        .unwrap();

    // Send Token to Auction
    let auction: MockAuction = app.query_ado_by_component_name(&router, auction_component.name);
    let start_time = router.block_info().time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO + 100;
    cw721
        .execute_send_nft(
            &mut router,
            owner.clone(),
            AndrAddr::from_string("./auction".to_string()),
            "0",
            &mock_start_auction(
                Some(Expiry::AtTime(Milliseconds(start_time))),
                Expiry::AtTime(Milliseconds(start_time + 2)),
                None,
                Asset::Cw20Token(AndrAddr::from_string(cw20.addr().to_string())),
                None,
                None,
                Some(vec![buyer_one.clone(), buyer_two.clone()]),
                None,
            ),
        )
        .unwrap();

    router.set_block(BlockInfo {
        height: router.block_info().height,
        time: Timestamp::from_nanos(start_time * MILLISECONDS_TO_NANOSECONDS_RATIO),
        chain_id: router.block_info().chain_id,
    });

    // Query Auction State
    let auction_ids =
        auction.query_auction_ids(&mut router, "0".to_string(), cw721.addr().to_string());
    assert_eq!(auction_ids.len(), 1);

    let auction_id = auction_ids.first().unwrap();
    let auction_state: AuctionStateResponse = auction.query_auction_state(&mut router, *auction_id);
    assert_eq!(auction_state.coin_denom, cw20.addr().to_string());

    // Place Bid One
    let hook_msg = Cw20HookMsg::PlaceBid {
        token_id: "0".to_owned(),
        token_address: cw721.addr().clone().to_string(),
    };
    cw20.execute_send(
        &mut router,
        buyer_one.clone(),
        auction.addr(),
        Uint128::new(50),
        &hook_msg,
    )
    .unwrap();

    // Check Bid Status One
    let bids_resp = auction.query_bids(&mut router, *auction_id);
    assert_eq!(bids_resp.len(), 1);

    let bid = bids_resp.first().unwrap();
    assert_eq!(bid.bidder, buyer_one.to_string());
    assert_eq!(bid.amount, Uint128::from(50u128));

    // Second bid by buyer_two
    cw20.execute_send(
        &mut router,
        buyer_two.clone(),
        auction.addr(),
        Uint128::new(100),
        &hook_msg,
    )
    .unwrap();

    // Check Bid Status One
    let bids_resp = auction.query_bids(&mut router, *auction_id);
    assert_eq!(bids_resp.len(), 2);

    let bid_two = bids_resp.get(1).unwrap();
    assert_eq!(bid_two.bidder, buyer_two.to_string());
    assert_eq!(bid_two.amount, Uint128::from(100u128));

    // End Auction
    router.set_block(BlockInfo {
        height: router.block_info().height,
        time: Timestamp::from_nanos((start_time + 1001) * MILLISECONDS_TO_NANOSECONDS_RATIO),
        chain_id: router.block_info().chain_id,
    });
    auction
        .execute_claim_auction(
            &mut router,
            buyer_two.clone(),
            "0".to_string(),
            cw721.addr().to_string(),
        )
        .unwrap();

    // Check Final State
    let owner_resp = cw721.query_owner_of(&router, "0".to_string());
    assert_eq!(owner_resp, buyer_two.to_string());

    // The auction's owner sold the NFT for 100, so the balance should increase by 100
    let cw20_balance = cw20.query_balance(&router, owner);
    assert_eq!(
        cw20_balance,
        owner_original_balance
            .checked_add(Uint128::new(100))
            .unwrap()
    );

    // Buyer two won the auction with a bid of 100, the balance should be 100 less than the original balance
    let cw20_balance = cw20.query_balance(&router, buyer_two);
    assert_eq!(
        cw20_balance,
        buyer_two_original_balance
            .checked_sub(Uint128::new(100))
            .unwrap()
    );

    // Buyer one was outbid, so the balance should remain unchanged
    let cw20_balance = cw20.query_balance(&router, buyer_one);
    assert_eq!(cw20_balance, buyer_one_original_balance);

    //
    //
    // Create a new auction with another cw20 set as the denom
    //
    //

    let second_cw20: MockCW20 =
        app.query_ado_by_component_name(&router, second_cw20_component.name);

    // Send Token to Auction
    let start_time = router.block_info().time.nanos() / MILLISECONDS_TO_NANOSECONDS_RATIO + 100;
    cw721
        .execute_send_nft(
            &mut router,
            owner.clone(),
            AndrAddr::from_string("./auction".to_string()),
            "1",
            &mock_start_auction(
                Some(Expiry::AtTime(Milliseconds(start_time))),
                Expiry::AtTime(Milliseconds(start_time + 2)),
                None,
                Asset::Cw20Token(AndrAddr::from_string(second_cw20.addr().to_string())),
                None,
                None,
                Some(vec![buyer_one.clone(), buyer_two.clone()]),
                None,
            ),
        )
        .unwrap();

    router.set_block(BlockInfo {
        height: router.block_info().height,
        time: Timestamp::from_nanos(start_time * MILLISECONDS_TO_NANOSECONDS_RATIO),
        chain_id: router.block_info().chain_id,
    });

    // Query Auction State
    let auction_ids =
        auction.query_auction_ids(&mut router, "1".to_string(), cw721.addr().to_string());
    assert_eq!(auction_ids.len(), 1);

    let auction_id = auction_ids.first().unwrap();
    let auction_state: AuctionStateResponse = auction.query_auction_state(&mut router, *auction_id);
    assert_eq!(auction_state.coin_denom, second_cw20.addr().to_string());

    // Place Bid One
    let hook_msg = Cw20HookMsg::PlaceBid {
        token_id: "1".to_owned(),
        token_address: cw721.addr().clone().to_string(),
    };
    second_cw20
        .execute_send(
            &mut router,
            buyer_one.clone(),
            auction.addr(),
            Uint128::new(50),
            &hook_msg,
        )
        .unwrap();

    // Check Bid Status One
    let bids_resp = auction.query_bids(&mut router, *auction_id);
    assert_eq!(bids_resp.len(), 1);

    let bid = bids_resp.first().unwrap();
    assert_eq!(bid.bidder, buyer_one.to_string());
    assert_eq!(bid.amount, Uint128::from(50u128));

    // Second bid by buyer_two
    second_cw20
        .execute_send(
            &mut router,
            buyer_two.clone(),
            auction.addr(),
            Uint128::new(100),
            &hook_msg,
        )
        .unwrap();

    // Check Bid Status One
    let bids_resp = auction.query_bids(&mut router, *auction_id);
    assert_eq!(bids_resp.len(), 2);

    let bid_two = bids_resp.get(1).unwrap();
    assert_eq!(bid_two.bidder, buyer_two.to_string());
    assert_eq!(bid_two.amount, Uint128::from(100u128));

    // End Auction
    router.set_block(BlockInfo {
        height: router.block_info().height,
        time: Timestamp::from_nanos((start_time + 1001) * MILLISECONDS_TO_NANOSECONDS_RATIO),
        chain_id: router.block_info().chain_id,
    });
    auction
        .execute_claim_auction(
            &mut router,
            buyer_two.clone(),
            "1".to_string(),
            cw721.addr().to_string(),
        )
        .unwrap();

    // Check Final State
    let owner_resp = cw721.query_owner_of(&router, "1".to_string());
    assert_eq!(owner_resp, buyer_two.to_string());

    // The auction's owner sold the NFT for 100, so the balance should increase by 100
    let cw20_balance = second_cw20.query_balance(&router, owner);
    assert_eq!(
        cw20_balance,
        owner_original_balance
            .checked_add(Uint128::new(100))
            .unwrap()
    );

    // Buyer two won the auction with a bid of 100, the balance should be 100 less than the original balance
    let cw20_balance = second_cw20.query_balance(&router, buyer_two);
    assert_eq!(
        cw20_balance,
        buyer_two_original_balance
            .checked_sub(Uint128::new(100))
            .unwrap()
    );

    // Buyer one was outbid, so the balance should remain unchanged
    let cw20_balance = second_cw20.query_balance(&router, buyer_one);
    assert_eq!(cw20_balance, buyer_one_original_balance);
}
