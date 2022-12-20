#![cfg(not(target_arch = "wasm32"))]

use andromeda_address_list::mock::{
    mock_add_address_msg, mock_address_list_instantiate_msg, mock_andromeda_address_list,
};
use andromeda_app::app::AppComponent;
use andromeda_app_contract::mock::{
    mock_andromeda_app, mock_app_instantiate_msg, mock_claim_ownership_msg, mock_get_address_msg,
    mock_get_components_msg,
};
use andromeda_cw721::mock::{
    mock_andromeda_cw721, mock_cw721_instantiate_msg, mock_cw721_owner_of, mock_quick_mint_msg,
    mock_send_nft,
};
use andromeda_marketplace::mock::{
    mock_andromeda_marketplace, mock_buy_token, mock_marketplace_instantiate_msg, mock_start_sale,
};
use andromeda_modules::rates::{Rate, RateInfo};

use andromeda_rates::mock::{mock_andromeda_rates, mock_rates_instantiate_msg};
use andromeda_testing::mock::MockAndromeda;
use common::ado_base::{modules::Module, recipient::Recipient};
use cosmwasm_std::{coin, to_binary, Addr, Uint128};
use cw721::OwnerOfResponse;
use cw_multi_test::{App, Executor};

fn mock_app() -> App {
    App::new(|router, _api, storage| {
        router
            .bank
            .init_balance(
                storage,
                &Addr::unchecked("owner"),
                [coin(999999, "uandr")].to_vec(),
            )
            .unwrap();
        router
            .bank
            .init_balance(
                storage,
                &Addr::unchecked("buyer"),
                [coin(200, "uandr")].to_vec(),
            )
            .unwrap();
    })
}

fn mock_andromeda(app: &mut App, admin_address: Addr) -> MockAndromeda {
    MockAndromeda::new(app, &admin_address)
}

#[test]
fn test_auction_app() {
    let owner = Addr::unchecked("owner");
    let buyer = Addr::unchecked("buyer");
    let rates_receiver = Addr::unchecked("receiver");

    let mut router = mock_app();
    let andr = mock_andromeda(&mut router, owner.clone());

    // Store contract codes
    let cw721_code_id = router.store_code(mock_andromeda_cw721());
    let marketplace_code_id = router.store_code(mock_andromeda_marketplace());
    let app_code_id = router.store_code(mock_andromeda_app());
    let rates_code_id = router.store_code(mock_andromeda_rates());
    let address_list_code_id = router.store_code(mock_andromeda_address_list());

    andr.store_code_id(&mut router, "cw721", cw721_code_id);
    andr.store_code_id(&mut router, "marketplace", marketplace_code_id);
    andr.store_code_id(&mut router, "rates", rates_code_id);
    andr.store_code_id(&mut router, "address-list", address_list_code_id);
    andr.store_code_id(&mut router, "app", app_code_id);

    // Generate App Components
    let cw721_init_msg = mock_cw721_instantiate_msg(
        "Test Tokens".to_string(),
        "TT".to_string(),
        owner.to_string(),
        None,
    );
    let cw721_component = AppComponent::new(
        "1".to_string(),
        "cw721".to_string(),
        to_binary(&cw721_init_msg).unwrap(),
    );

    let rates: Vec<RateInfo> = vec![RateInfo {
        rate: Rate::Flat(coin(100, "uandr")),
        is_additive: true,
        description: None,
        recipients: vec![Recipient::Addr(rates_receiver.to_string())],
    }];
    let rates_init_msg = mock_rates_instantiate_msg(rates);
    let rates_component = AppComponent::new("2", "rates", to_binary(&rates_init_msg).unwrap());

    let address_list_init_msg = mock_address_list_instantiate_msg(true);
    let address_list_component = AppComponent::new(
        "3",
        "address-list",
        to_binary(&address_list_init_msg).unwrap(),
    );

    let modules: Vec<Module> = vec![
        Module::new("rates", rates_component.clone().name, false),
        Module::new("address-list", address_list_component.clone().name, false),
    ];
    let marketplace_init_msg = mock_marketplace_instantiate_msg(Some(modules));
    let marketplace_component = AppComponent::new(
        "4".to_string(),
        "marketplace".to_string(),
        to_binary(&marketplace_init_msg).unwrap(),
    );

    // Create App
    let app_components = vec![
        cw721_component.clone(),
        rates_component,
        address_list_component.clone(),
        marketplace_component.clone(),
    ];
    let app_init_msg = mock_app_instantiate_msg(
        "Auction App".to_string(),
        app_components.clone(),
        andr.registry_address.to_string(),
    );

    let app_addr = router
        .instantiate_contract(
            app_code_id,
            owner.clone(),
            &app_init_msg,
            &[],
            "Auction App",
            Some(owner.to_string()),
        )
        .unwrap();

    let components: Vec<AppComponent> = router
        .wrap()
        .query_wasm_smart(app_addr.clone(), &mock_get_components_msg())
        .unwrap();

    assert_eq!(components, app_components);

    // Claim Ownership
    router
        .execute_contract(
            owner.clone(),
            app_addr.clone(),
            &mock_claim_ownership_msg(None),
            &[],
        )
        .unwrap();

    let cw721_addr: String = router
        .wrap()
        .query_wasm_smart(
            app_addr.clone(),
            &mock_get_address_msg(cw721_component.name),
        )
        .unwrap();
    let marketplace_addr: String = router
        .wrap()
        .query_wasm_smart(
            app_addr.clone(),
            &mock_get_address_msg(marketplace_component.name),
        )
        .unwrap();
    let address_list_addr: String = router
        .wrap()
        .query_wasm_smart(app_addr, &mock_get_address_msg(address_list_component.name))
        .unwrap();

    // Mint Tokens
    let mint_msg = mock_quick_mint_msg(1, owner.to_string());
    router
        .execute_contract(
            owner.clone(),
            Addr::unchecked(cw721_addr.clone()),
            &mint_msg,
            &[],
        )
        .unwrap();

    let token_id = "0";

    // Whitelist
    router
        .execute_contract(
            owner.clone(),
            Addr::unchecked(address_list_addr.clone()),
            &mock_add_address_msg(cw721_addr.to_string()),
            &[],
        )
        .unwrap();
    router
        .execute_contract(
            owner.clone(),
            Addr::unchecked(address_list_addr),
            &mock_add_address_msg(buyer.to_string()),
            &[],
        )
        .unwrap();

    // Send Token to Marketplace
    let send_nft_msg = mock_send_nft(
        marketplace_addr.clone(),
        token_id.to_string(),
        to_binary(&mock_start_sale(Uint128::from(100u128), "uandr")).unwrap(),
    );
    router
        .execute_contract(
            owner,
            Addr::unchecked(cw721_addr.clone()),
            &send_nft_msg,
            &[],
        )
        .unwrap();

    // Buy Token
    let buy_msg = mock_buy_token(cw721_addr.clone(), token_id);
    router
        .execute_contract(
            buyer.clone(),
            Addr::unchecked(marketplace_addr),
            &buy_msg,
            &[coin(200, "uandr")],
        )
        .unwrap();

    // Check final state
    let owner_resp: OwnerOfResponse = router
        .wrap()
        .query_wasm_smart(cw721_addr, &mock_cw721_owner_of(token_id.to_string(), None))
        .unwrap();
    assert_eq!(owner_resp.owner, buyer.to_string());

    let balance = router
        .wrap()
        .query_balance(rates_receiver, "uandr")
        .unwrap();
    assert_eq!(balance.amount, Uint128::from(100u128));
}
