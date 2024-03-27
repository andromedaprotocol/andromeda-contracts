#![cfg(not(target_arch = "wasm32"))]

use andromeda_data_storage::primitive::{GetValueResponse, Primitive};

use andromeda_primitive::mock::{
    mock_andromeda_primitive, mock_primitive_get_value, mock_primitive_instantiate_msg,
    mock_store_value_msg,
};
use andromeda_testing::mock::{mock_app, MockAndromeda, MockApp};
use cosmwasm_schema::schemars::Map;
use cosmwasm_std::Addr;
use cw_multi_test::Executor;

fn mock_andromeda(app: &mut MockApp, admin_address: Addr) -> MockAndromeda {
    MockAndromeda::new(app, &admin_address)
}

#[test]
fn test_primtive() {
    let mut router = mock_app();
    let sender = router.api().addr_make("owner");

    let andr = mock_andromeda(&mut router, sender.clone());

    // Store contract codes
    let primtive_code_id = router.store_code(mock_andromeda_primitive());

    andr.store_code_id(&mut router, "primitve", primtive_code_id);

    let primitive_init_msg = mock_primitive_instantiate_msg(
        andr.kernel.addr().to_string(),
        None,
        andromeda_data_storage::primitive::PrimitiveRestriction::Private,
    );

    let primitive_addr = router
        .instantiate_contract(
            primtive_code_id,
            sender.clone(),
            &primitive_init_msg,
            &[],
            "Auction App",
            Some(sender.to_string()),
        )
        .unwrap();

    let mut map = Map::new();
    map.insert("bool".to_string(), Primitive::Bool(true));
    map.insert(
        "vec".into(),
        Primitive::Vec(vec![Primitive::String("My String".to_string())]),
    );
    map.insert("object".into(), Primitive::Object(map.clone()));

    let value = Primitive::Object(map.clone());
    // Claim Ownership
    router
        .execute_contract(
            sender,
            primitive_addr.clone(),
            &mock_store_value_msg(Some("key".to_string()), value.clone()),
            &[],
        )
        .unwrap();

    // Check final state
    let get_value_resp: GetValueResponse = router
        .wrap()
        .query_wasm_smart(
            primitive_addr,
            &mock_primitive_get_value(Some("key".to_string())),
        )
        .unwrap();
    assert_eq!(get_value_resp.value, value);
}
