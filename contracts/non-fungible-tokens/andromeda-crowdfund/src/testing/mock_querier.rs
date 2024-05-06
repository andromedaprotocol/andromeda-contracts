use andromeda_non_fungible_tokens::{
    crowdfund::{CampaignConfig, Tier, TierMetaData},
    cw721::TokenExtension,
};
use andromeda_std::{
    ado_base::InstantiateMsg,
    ado_contract::ADOContract,
    amp::AndrAddr,
    testing::mock_querier::{WasmMockQuerier, MOCK_ADO_PUBLISHER, MOCK_KERNEL_CONTRACT},
};
use cosmwasm_std::{
    testing::{mock_env, mock_info, MockApi, MockQuerier, MockStorage},
    Coin, OwnedDeps, QuerierWrapper, Uint128, Uint64,
};

pub const MOCK_TIER_CONTRACT: &str = "tier_contract";
pub const MOCK_WITHDRAWAL_ADDRESS: &str = "withdrawal_address";

pub fn mock_campaign_config() -> CampaignConfig {
    CampaignConfig {
        title: "First Crowdfund".to_string(),
        description: "Demo campaign for testing".to_string(),
        banner: "http://<campaign_banner>".to_string(),
        url: "http://<campaign_url>".to_string(),
        denom: "uandr".to_string(),
        tier_address: AndrAddr::from_string(MOCK_TIER_CONTRACT.to_owned()),
        withdrawal_address: AndrAddr::from_string(MOCK_WITHDRAWAL_ADDRESS.to_owned()),
        soft_cap: None,
        hard_cap: None,
    }
}

pub fn mock_campaign_tiers() -> Vec<Tier> {
    vec![Tier {
        level: Uint64::zero(),
        limit: None,
        price: Uint128::new(10u128),
        meta_data: TierMetaData {
            extension: TokenExtension {
                publisher: MOCK_ADO_PUBLISHER.to_string(),
            },
            owner: None,
            token_uri: None,
        },
    }]
}

/// Alternative to `cosmwasm_std::testing::mock_dependencies` that allows us to respond to custom queries.
///
/// Automatically assigns a kernel address as MOCK_KERNEL_CONTRACT.
pub fn mock_dependencies_custom(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(MOCK_TIER_CONTRACT, contract_balance)]));
    let storage = MockStorage::default();
    let mut deps = OwnedDeps {
        storage,
        api: MockApi::default(),
        querier: custom_querier,
        custom_query_type: std::marker::PhantomData,
    };
    ADOContract::default()
        .instantiate(
            &mut deps.storage,
            mock_env(),
            &deps.api,
            &QuerierWrapper::new(&deps.querier),
            mock_info("sender", &[]),
            InstantiateMsg {
                ado_type: "crowdfund".to_string(),
                ado_version: "test".to_string(),

                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();
    deps
}
