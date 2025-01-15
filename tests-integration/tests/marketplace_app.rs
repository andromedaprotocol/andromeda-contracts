use andromeda_address_list::mock::{
    mock_address_list_instantiate_msg, mock_andromeda_address_list, MockAddressList,
};
use andromeda_app::app::AppComponent;
use andromeda_app_contract::mock::{mock_andromeda_app, MockAppContract};
use andromeda_cw20::mock::{mock_andromeda_cw20, mock_cw20_instantiate_msg, mock_minter, MockCW20};
use andromeda_cw721::mock::{mock_andromeda_cw721, mock_cw721_instantiate_msg, MockCW721};
use andromeda_finance::splitter::AddressPercent;
use andromeda_marketplace::mock::{
    mock_andromeda_marketplace, mock_buy_token, mock_marketplace_instantiate_msg,
    mock_receive_packet, mock_start_sale, MockMarketplace,
};
use andromeda_non_fungible_tokens::marketplace::Cw20HookMsg;
use andromeda_rates::mock::{mock_andromeda_rates, mock_rates_instantiate_msg, MockRates};
use andromeda_splitter::mock::{
    mock_andromeda_splitter, mock_splitter_instantiate_msg, mock_splitter_send_msg,
};
use andromeda_std::ado_base::permissioning::{LocalPermission, Permission};
use andromeda_std::ado_base::rates::{AllRatesResponse, LocalRate};
use andromeda_std::ado_base::rates::{LocalRateType, LocalRateValue, PercentRate, Rate};
use andromeda_std::amp::messages::{AMPMsg, AMPPkt};
use andromeda_std::amp::{AndrAddr, Recipient};
use andromeda_std::common::denom::Asset;
use andromeda_std::error::ContractError;
use andromeda_testing::mock::mock_app;
use andromeda_testing::mock_builder::MockAndromedaBuilder;
use andromeda_testing::MockADO;
use andromeda_testing::MockContract;
use cosmwasm_std::{coin, to_json_binary, Addr, BlockInfo, Decimal, Uint128};
use cw20::Cw20Coin;
use cw_multi_test::Executor;

#[test]
fn test_marketplace_app() {
    let mut router = mock_app(None);
    let andr = MockAndromedaBuilder::new(&mut router, "admin")
        .with_wallets(vec![
            ("owner", vec![]),
            ("buyer", vec![coin(200, "uandr")]),
            ("receiver", vec![]),
        ])
        .with_contracts(vec![
            ("app-contract", mock_andromeda_app()),
            ("cw721", mock_andromeda_cw721()),
            ("marketplace", mock_andromeda_marketplace()),
            ("rates", mock_andromeda_rates()),
            ("address-list", mock_andromeda_address_list()),
        ])
        .build(&mut router);
    let owner = andr.get_wallet("owner");
    let buyer = andr.get_wallet("buyer");
    let rates_receiver = andr.get_wallet("receiver");

    // Generate App Components
    let cw721_init_msg = mock_cw721_instantiate_msg(
        "Test Tokens".to_string(),
        "TT".to_string(),
        owner.to_string(),
        andr.kernel.addr().to_string(),
        None,
    );
    let cw721_component = AppComponent::new(
        "tokens".to_string(),
        "cw721".to_string(),
        to_json_binary(&cw721_init_msg).unwrap(),
    );
    // Set a royalty which is worth as much as the marketplace sale price
    // The sale recipient will not receive any funds because they're all going to the royalty recipient
    let local_rate = LocalRate {
        rate_type: LocalRateType::Deductive,
        recipient: Recipient::from_string(rates_receiver.to_string()),
        value: LocalRateValue::Flat(coin(100, "uandr")),
        description: None,
    };

    let rates_init_msg = mock_rates_instantiate_msg(
        "Buy".to_string(),
        local_rate,
        andr.kernel.addr().to_string(),
        None,
    );
    let rates_component =
        AppComponent::new("rates", "rates", to_json_binary(&rates_init_msg).unwrap());

    let address_list_init_msg =
        mock_address_list_instantiate_msg(andr.kernel.addr().to_string(), None, None);

    let address_list_component = AppComponent::new(
        "address-list",
        "address-list",
        to_json_binary(&address_list_init_msg).unwrap(),
    );

    let marketplace_init_msg =
        mock_marketplace_instantiate_msg(andr.kernel.addr().to_string(), None, None, None);
    let marketplace_component = AppComponent::new(
        "marketplace".to_string(),
        "marketplace".to_string(),
        to_json_binary(&marketplace_init_msg).unwrap(),
    );

    // Create App
    let app_components = vec![
        cw721_component.clone(),
        rates_component.clone(),
        address_list_component.clone(),
        marketplace_component.clone(),
    ];
    let app_code_id = andr.get_code_id(&mut router, "app-contract");
    let app = MockAppContract::instantiate(
        app_code_id,
        owner,
        &mut router,
        "Marketplace App",
        app_components.clone(),
        andr.kernel.addr(),
        None,
    );

    let components = app.query_components(&router);
    assert_eq!(components, app_components);

    let cw721: MockCW721 = app.query_ado_by_component_name(&router, cw721_component.name);
    let marketplace: MockMarketplace =
        app.query_ado_by_component_name(&router, marketplace_component.name);
    let address_list: MockAddressList =
        app.query_ado_by_component_name(&router, address_list_component.name);
    let rates: MockRates = app.query_ado_by_component_name(&router, rates_component.name);

    // Set contract rate linked to the above rates contract
    marketplace
        .execute_set_rate(
            &mut router,
            owner.clone(),
            "Buy",
            Rate::Contract(AndrAddr::from_string(rates.addr())),
        )
        .unwrap();

    let rate = marketplace
        .query_rates(&mut router, "Buy".to_string())
        .unwrap();

    assert_eq!(rate, Rate::Contract(AndrAddr::from_string(rates.addr())));

    let all_rates: AllRatesResponse = marketplace.query_all_rates(&mut router);

    assert_eq!(
        all_rates,
        AllRatesResponse {
            all_rates: vec![(
                "Buy".to_string(),
                Rate::Contract(AndrAddr::from_string(rates.addr()))
            )]
        }
    );

    // Mint Tokens
    cw721
        .execute_quick_mint(&mut router, owner.clone(), 1, owner.to_string())
        .unwrap();
    let token_id = "0";

    // Send Token to Marketplace
    cw721
        .execute_send_nft(
            &mut router,
            owner.clone(),
            marketplace.addr().clone(),
            token_id,
            &mock_start_sale(
                Uint128::from(100u128),
                Asset::NativeToken("uandr".to_string()),
                None,
                None,
                None,
            ),
        )
        .unwrap();

    // Buy Token
    let buy_msg = mock_buy_token(cw721.addr().clone(), token_id);
    let amp_msg = AMPMsg::new(
        Addr::unchecked(marketplace.addr().clone()),
        to_json_binary(&buy_msg).unwrap(),
        Some(vec![coin(200, "uandr")]),
    );

    let packet = AMPPkt::new(buyer.clone(), buyer.clone(), vec![amp_msg]);
    let receive_packet_msg = mock_receive_packet(packet);

    let block_info = router.block_info();
    router.set_block(BlockInfo {
        height: block_info.height,
        time: block_info.time.plus_minutes(1),
        chain_id: block_info.chain_id,
    });

    // Try adding limited permission in address list, should error
    let err: ContractError = address_list
        .execute_actor_permission(
            &mut router,
            owner.clone(),
            vec![AndrAddr::from_string(buyer.clone())],
            LocalPermission::limited(None, None, 1),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(
        err,
        ContractError::InvalidPermission {
            msg: "Limited permission is not supported in address list contract".to_string(),
        }
    );

    // Blacklist buyer in address list
    address_list
        .execute_actor_permission(
            &mut router,
            owner.clone(),
            vec![AndrAddr::from_string(buyer.clone())],
            LocalPermission::blacklisted(None, None),
        )
        .unwrap();

    // Blacklist buyer using contract permission
    marketplace
        .execute_set_permissions(
            &mut router,
            owner.clone(),
            vec![AndrAddr::from_string(buyer.clone())],
            "Buy",
            Permission::Contract(AndrAddr::from_string(address_list.addr())),
        )
        .unwrap();

    // Should return Unauthorized error
    let err: ContractError = router
        .execute_contract(
            buyer.clone(),
            Addr::unchecked(marketplace.addr()),
            &mock_buy_token(cw721.addr(), token_id),
            // We're sending the exact amount required, which is the price + tax
            &[coin(100, "uandr")],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Unauthorized {});

    address_list
        .execute_actor_permission(
            &mut router,
            owner.clone(),
            vec![AndrAddr::from_string(buyer.clone())],
            LocalPermission::whitelisted(None, None),
        )
        .unwrap();

    router
        .execute_contract(
            buyer.clone(),
            Addr::unchecked(marketplace.addr()),
            &receive_packet_msg,
            // We're sending the exact amount required, which is the price + tax
            &[coin(100, "uandr")],
        )
        .unwrap();

    // Check final state
    let owner_of_token = cw721.query_owner_of(&router, token_id);
    assert_eq!(owner_of_token, buyer.to_string());

    let balance = router
        .wrap()
        .query_balance(rates_receiver, "uandr")
        .unwrap();
    assert_eq!(balance.amount, Uint128::from(100u128));

    let balance = router.wrap().query_balance(owner, "uandr").unwrap();
    assert_eq!(balance.amount, Uint128::zero());
}

#[test]
fn test_marketplace_app_recipient() {
    let mut router = mock_app(None);
    let andr = MockAndromedaBuilder::new(&mut router, "admin")
        .with_wallets(vec![
            ("owner", vec![]),
            ("buyer", vec![coin(200, "uandr")]),
            ("receiver", vec![]),
        ])
        .with_contracts(vec![
            ("app-contract", mock_andromeda_app()),
            ("cw721", mock_andromeda_cw721()),
            ("marketplace", mock_andromeda_marketplace()),
            ("splitter", mock_andromeda_splitter()),
            ("address-list", mock_andromeda_address_list()),
        ])
        .build(&mut router);
    let owner = andr.get_wallet("owner");
    let buyer = andr.get_wallet("buyer");
    let receiver = andr.get_wallet("receiver");

    // Generate App Components
    let cw721_init_msg = mock_cw721_instantiate_msg(
        "Test Tokens".to_string(),
        "TT".to_string(),
        owner.to_string(),
        andr.kernel.addr().to_string(),
        None,
    );
    let cw721_component = AppComponent::new(
        "tokens".to_string(),
        "cw721".to_string(),
        to_json_binary(&cw721_init_msg).unwrap(),
    );

    let splitter_init_msg = mock_splitter_instantiate_msg(
        vec![AddressPercent::new(
            Recipient::from_string(receiver),
            Decimal::one(),
        )],
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

    let marketplace_init_msg =
        mock_marketplace_instantiate_msg(andr.kernel.addr().to_string(), None, None, None);

    let marketplace_component = AppComponent::new(
        "marketplace".to_string(),
        "marketplace".to_string(),
        to_json_binary(&marketplace_init_msg).unwrap(),
    );

    // Create App
    let app_components = vec![
        cw721_component.clone(),
        splitter_component.clone(),
        marketplace_component.clone(),
    ];
    let app_code_id = andr.get_code_id(&mut router, "app-contract");
    let app = MockAppContract::instantiate(
        app_code_id,
        owner,
        &mut router,
        "Marketplace App",
        app_components.clone(),
        andr.kernel.addr(),
        None,
    );

    let components = app.query_components(&router);
    assert_eq!(components, app_components);

    let cw721: MockCW721 = app.query_ado_by_component_name(&router, cw721_component.name);
    let marketplace: MockMarketplace =
        app.query_ado_by_component_name(&router, marketplace_component.name);

    // Mint Tokens
    cw721
        .execute_quick_mint(&mut router, owner.clone(), 1, owner.to_string())
        .unwrap();
    let token_id = "0";

    // Send Token to Marketplace
    cw721
        .execute_send_nft(
            &mut router,
            owner.clone(),
            marketplace.addr().clone(),
            token_id,
            &mock_start_sale(
                Uint128::from(100u128),
                Asset::NativeToken("uandr".to_string()),
                None,
                None,
                Some(
                    Recipient::from_string(format!("./{}", splitter_component.name))
                        .with_msg(mock_splitter_send_msg(None)),
                ),
            ),
        )
        .unwrap();

    // Buy Token
    let buy_msg = mock_buy_token(cw721.addr().clone(), token_id);
    let amp_msg = AMPMsg::new(
        Addr::unchecked(marketplace.addr().clone()),
        to_json_binary(&buy_msg).unwrap(),
        Some(vec![coin(200, "uandr")]),
    );

    let packet = AMPPkt::new(buyer.clone(), buyer.clone(), vec![amp_msg]);
    let receive_packet_msg = mock_receive_packet(packet);

    let block_info = router.block_info();
    router.set_block(BlockInfo {
        height: block_info.height,
        time: block_info.time.plus_minutes(1),
        chain_id: block_info.chain_id,
    });

    router
        .execute_contract(
            buyer.clone(),
            Addr::unchecked(marketplace.addr()),
            &receive_packet_msg,
            &[coin(100, "uandr")],
        )
        .unwrap();

    // Check final state
    let owner_of_token = cw721.query_owner_of(&router, token_id);
    assert_eq!(owner_of_token, buyer.to_string());

    let balance = router.wrap().query_balance(receiver, "uandr").unwrap();
    assert_eq!(balance.amount, Uint128::from(100u128));
}

#[test]
fn test_marketplace_app_cw20_restricted() {
    let mut router = mock_app(None);
    let andr = MockAndromedaBuilder::new(&mut router, "admin")
        .with_wallets(vec![
            ("owner", vec![]),
            ("buyer", vec![coin(200, "uandr")]),
            ("receiver", vec![]),
        ])
        .with_contracts(vec![
            ("app-contract", mock_andromeda_app()),
            ("cw721", mock_andromeda_cw721()),
            ("cw20", mock_andromeda_cw20()),
            ("marketplace", mock_andromeda_marketplace()),
            ("rates", mock_andromeda_rates()),
            ("address-list", mock_andromeda_address_list()),
        ])
        .build(&mut router);
    let owner = andr.get_wallet("owner");
    let buyer = andr.get_wallet("buyer");
    let rates_receiver = andr.get_wallet("receiver");
    // Generate App Components
    let cw721_init_msg = mock_cw721_instantiate_msg(
        "Test Tokens".to_string(),
        "TT".to_string(),
        owner.to_string(),
        andr.kernel.addr().to_string(),
        None,
    );
    let cw721_component = AppComponent::new(
        "tokens".to_string(),
        "cw721".to_string(),
        to_json_binary(&cw721_init_msg).unwrap(),
    );

    let owner_original_balance = Uint128::new(1_000);
    let buyer_original_balance = Uint128::new(2_000);
    let initial_balances = vec![
        Cw20Coin {
            address: owner.to_string(),
            amount: owner_original_balance,
        },
        Cw20Coin {
            address: buyer.to_string(),
            amount: buyer_original_balance,
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

    let address_list_init_msg =
        mock_address_list_instantiate_msg(andr.kernel.addr().to_string(), None, None);

    let address_list_component = AppComponent::new(
        "address-list",
        "address-list",
        to_json_binary(&address_list_init_msg).unwrap(),
    );

    let marketplace_init_msg = mock_marketplace_instantiate_msg(
        andr.kernel.addr().to_string(),
        None,
        Some(vec![AndrAddr::from_string(format!(
            "./{}",
            cw20_component.name
        ))]),
        None,
    );
    let marketplace_component = AppComponent::new(
        "marketplace".to_string(),
        "marketplace".to_string(),
        to_json_binary(&marketplace_init_msg).unwrap(),
    );

    // Create App
    let app_components = vec![
        cw721_component.clone(),
        cw20_component.clone(),
        second_cw20_component.clone(),
        address_list_component.clone(),
        marketplace_component.clone(),
    ];

    let app_code_id = andr.get_code_id(&mut router, "app-contract");
    let app = MockAppContract::instantiate(
        app_code_id,
        owner,
        &mut router,
        "Marketplace App",
        app_components.clone(),
        andr.kernel.addr(),
        None,
    );

    let components = app.query_components(&router);
    assert_eq!(components, app_components);

    let cw721: MockCW721 = app.query_ado_by_component_name(&router, cw721_component.name);
    let marketplace: MockMarketplace =
        app.query_ado_by_component_name(&router, marketplace_component.name);
    let address_list: MockAddressList =
        app.query_ado_by_component_name(&router, address_list_component.name);
    let cw20: MockCW20 = app.query_ado_by_component_name(&router, cw20_component.name.clone());

    let local_rate = LocalRate {
        rate_type: LocalRateType::Additive,
        recipient: Recipient::from_string(rates_receiver.to_string()),
        // This is the cw20's address
        value: LocalRateValue::Flat(coin(100, format!("./{}", cw20_component.name))),
        description: None,
    };

    let rates_init_msg = mock_rates_instantiate_msg(
        "Buy".to_string(),
        local_rate,
        andr.kernel.addr().to_string(),
        None,
    );
    let rates_component =
        AppComponent::new("rates", "rates", to_json_binary(&rates_init_msg).unwrap());

    app.execute_add_app_component(&mut router, owner.clone(), rates_component.clone())
        .unwrap();

    let rates: MockRates = app.query_ado_by_component_name(&router, rates_component.name);

    // Set contract rate linked to the above rates contract
    marketplace
        .execute_set_rate(
            &mut router,
            owner.clone(),
            "Buy",
            Rate::Contract(AndrAddr::from_string(rates.addr())),
        )
        .unwrap();

    // Mint Tokens
    cw721
        .execute_quick_mint(&mut router, owner.clone(), 1, owner.to_string())
        .unwrap();
    let token_id = "0";

    // Whitelist
    address_list
        .execute_actor_permission(
            &mut router,
            owner.clone(),
            vec![
                AndrAddr::from_string(cw721.addr().clone()),
                AndrAddr::from_string(cw20.addr().clone()),
                AndrAddr::from_string(buyer.clone()),
                AndrAddr::from_string(owner.clone()),
            ],
            LocalPermission::whitelisted(None, None),
        )
        .unwrap();

    cw721
        .execute_send_nft(
            &mut router,
            owner.clone(),
            marketplace.addr(),
            token_id,
            &mock_start_sale(
                Uint128::from(100u128),
                Asset::Cw20Token(AndrAddr::from_string(cw20.addr().clone())),
                None,
                None,
                None,
            ),
        )
        .unwrap();

    // Try updating denom to another unpermissioned cw20, shouldn't work since this a restricted cw20 sale
    let second_cw20: MockCW20 =
        app.query_ado_by_component_name(&router, second_cw20_component.name);

    let err: ContractError = marketplace
        .execute_update_sale(
            &mut router,
            owner.clone(),
            token_id.to_string(),
            cw721.addr().to_string(),
            // This cw20 hasn't been permissioned
            Asset::Cw20Token(AndrAddr::from_string(second_cw20.addr().to_string())),
            Uint128::new(100),
            None,
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

    let block_info = router.block_info();
    router.set_block(BlockInfo {
        height: block_info.height,
        time: block_info.time.plus_minutes(1),
        chain_id: block_info.chain_id,
    });

    // Buy Token
    let hook_msg = Cw20HookMsg::Buy {
        token_id: token_id.to_owned(),
        token_address: cw721.addr().to_string(),
    };
    cw20.execute_send(
        &mut router,
        buyer.clone(),
        marketplace.addr(),
        Uint128::new(200),
        &hook_msg,
    )
    .unwrap();

    let owner_resp = cw721.query_owner_of(&router, token_id.to_string());
    assert_eq!(owner_resp, buyer);

    // The NFT owner sold it for 200, there's also a 50% tax so the owner should receive 100
    let cw20_balance_response = cw20.query_balance(&router, owner);
    assert_eq!(
        cw20_balance_response,
        owner_original_balance
            .checked_add(Uint128::new(100))
            .unwrap()
    );

    // Buyer bought the NFT for 200, should be 200 less
    let cw20_balance_response = cw20.query_balance(&router, buyer);
    assert_eq!(
        cw20_balance_response,
        buyer_original_balance
            .checked_sub(Uint128::new(200))
            .unwrap()
    );

    // The rates receiver should get 100 coins
    let cw20_balance_response = cw20.query_balance(&router, rates_receiver);
    assert_eq!(cw20_balance_response, Uint128::new(100));
}

#[test]
fn test_marketplace_app_cw20_unrestricted() {
    let mut router = mock_app(None);
    let andr = MockAndromedaBuilder::new(&mut router, "admin")
        .with_wallets(vec![
            ("owner", vec![]),
            ("buyer", vec![coin(200, "uandr")]),
            ("receiver", vec![]),
        ])
        .with_contracts(vec![
            ("app-contract", mock_andromeda_app()),
            ("cw721", mock_andromeda_cw721()),
            ("cw20", mock_andromeda_cw20()),
            ("marketplace", mock_andromeda_marketplace()),
            ("rates", mock_andromeda_rates()),
            ("address-list", mock_andromeda_address_list()),
        ])
        .build(&mut router);
    let owner = andr.get_wallet("owner");
    let buyer = andr.get_wallet("buyer");
    let rates_receiver = andr.get_wallet("receiver");
    // Generate App Components
    let cw721_init_msg = mock_cw721_instantiate_msg(
        "Test Tokens".to_string(),
        "TT".to_string(),
        owner.to_string(),
        andr.kernel.addr().to_string(),
        None,
    );
    let cw721_component = AppComponent::new(
        "tokens".to_string(),
        "cw721".to_string(),
        to_json_binary(&cw721_init_msg).unwrap(),
    );

    let owner_original_balance = Uint128::new(1_000);
    let buyer_original_balance = Uint128::new(2_000);
    let initial_balances = vec![
        Cw20Coin {
            address: owner.to_string(),
            amount: owner_original_balance,
        },
        Cw20Coin {
            address: buyer.to_string(),
            amount: buyer_original_balance,
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

    // set rates for the second cw20 later
    let local_rate = LocalRate {
        rate_type: LocalRateType::Additive,
        recipient: Recipient::from_string(rates_receiver.to_string()),
        // This is the cw20's address
        value: LocalRateValue::Percent(PercentRate {
            percent: Decimal::percent(20),
        }),
        description: None,
    };

    let rates_init_msg = mock_rates_instantiate_msg(
        "Buy".to_string(),
        local_rate,
        andr.kernel.addr().to_string(),
        None,
    );
    let rates_component =
        AppComponent::new("rates", "rates", to_json_binary(&rates_init_msg).unwrap());

    let address_list_init_msg =
        mock_address_list_instantiate_msg(andr.kernel.addr().to_string(), None, None);

    let address_list_component = AppComponent::new(
        "address-list",
        "address-list",
        to_json_binary(&address_list_init_msg).unwrap(),
    );

    let marketplace_init_msg =
        mock_marketplace_instantiate_msg(andr.kernel.addr().to_string(), None, None, None);
    let marketplace_component = AppComponent::new(
        "marketplace".to_string(),
        "marketplace".to_string(),
        to_json_binary(&marketplace_init_msg).unwrap(),
    );

    // Create App
    let app_components = vec![
        cw721_component.clone(),
        cw20_component.clone(),
        second_cw20_component.clone(),
        rates_component.clone(),
        address_list_component.clone(),
        marketplace_component.clone(),
    ];

    let app_code_id = andr.get_code_id(&mut router, "app-contract");
    let app = MockAppContract::instantiate(
        app_code_id,
        owner,
        &mut router,
        "Marketplace App",
        app_components.clone(),
        andr.kernel.addr(),
        None,
    );

    let components = app.query_components(&router);
    assert_eq!(components, app_components);

    let cw721: MockCW721 = app.query_ado_by_component_name(&router, cw721_component.name);
    let marketplace: MockMarketplace =
        app.query_ado_by_component_name(&router, marketplace_component.name);
    let address_list: MockAddressList =
        app.query_ado_by_component_name(&router, address_list_component.name);

    let cw20: MockCW20 = app.query_ado_by_component_name(&router, cw20_component.name);
    let rates: MockRates = app.query_ado_by_component_name(&router, rates_component.name);

    // Set contract rate linked to the above rates contract,
    marketplace
        .execute_set_rate(
            &mut router,
            owner.clone(),
            "Buy",
            Rate::Contract(AndrAddr::from_string(rates.addr())),
        )
        .unwrap();

    // Mint Tokens
    cw721
        .execute_quick_mint(&mut router, owner.clone(), 1, owner.to_string())
        .unwrap();

    let token_id = "0";

    // Whitelist

    let second_cw20: MockCW20 =
        app.query_ado_by_component_name(&router, second_cw20_component.name);

    address_list
        .execute_actor_permission(
            &mut router,
            owner.clone(),
            vec![
                AndrAddr::from_string(cw721.addr().clone()),
                AndrAddr::from_string(cw20.addr().clone()),
                AndrAddr::from_string(second_cw20.addr().clone()),
                AndrAddr::from_string(buyer.clone()),
                AndrAddr::from_string(owner.clone()),
            ],
            LocalPermission::whitelisted(None, None),
        )
        .unwrap();

    // Send Token to Marketplace
    cw721
        .execute_send_nft(
            &mut router,
            owner.clone(),
            marketplace.addr(),
            token_id.to_string(),
            &mock_start_sale(
                Uint128::from(100u128),
                Asset::Cw20Token(AndrAddr::from_string(cw20.addr().clone())),
                None,
                None,
                None,
            ),
        )
        .unwrap();
    let _local_rate2 = LocalRate {
        rate_type: LocalRateType::Additive,
        recipient: Recipient::from_string(rates_receiver.to_string()),
        // This is the cw20's address
        value: LocalRateValue::Flat(coin(
            100,
            "andr1ywhkkafy0jgr3etypp40v6ct9ffmvakrsruwvp595pd9juv5tafqqzph5h",
        )),
        description: None,
    };

    // Try updating denom to another unpermissioned cw20, should work since this an unrestricted cw20 sale
    marketplace
        .execute_update_sale(
            &mut router,
            owner.clone(),
            cw721.addr().to_string(),
            token_id.to_string(),
            Asset::Cw20Token(AndrAddr::from_string(second_cw20.addr().to_string())),
            Uint128::new(100),
            None,
        )
        .unwrap();

    let block_info = router.block_info();
    router.set_block(BlockInfo {
        height: block_info.height,
        time: block_info.time.plus_minutes(1),
        chain_id: block_info.chain_id,
    });

    // Buy Token
    let hook_msg = Cw20HookMsg::Buy {
        token_id: token_id.to_owned(),
        token_address: cw721.addr().to_string(),
    };
    second_cw20
        .execute_send(
            &mut router,
            buyer.clone(),
            marketplace.addr(),
            // Send the exact amount needed. 100 + 20 for tax
            Uint128::new(120),
            &hook_msg,
        )
        .unwrap();

    // Check final state
    let owner_of_token = cw721.query_owner_of(&router, token_id);
    assert_eq!(owner_of_token, buyer.to_string());

    // The NFT owner sold it for 200, there's also a 50% tax so the owner should receive 100
    let second_cw20_balance_response = second_cw20.query_balance(&router, owner);
    assert_eq!(
        second_cw20_balance_response,
        owner_original_balance
            .checked_add(Uint128::new(100))
            .unwrap()
    );

    // Buyer bought the NFT for 120, should be 120 less
    let second_cw20_balance_response = second_cw20.query_balance(&router, buyer);
    assert_eq!(
        second_cw20_balance_response,
        buyer_original_balance
            .checked_sub(Uint128::new(120))
            .unwrap()
    );

    // The rates receiver should get 20 coins because it's 20% tax on 100
    let second_cw20_balance_response = second_cw20.query_balance(&router, rates_receiver);
    assert_eq!(second_cw20_balance_response, Uint128::new(20));
}
