mod ibc_tests_setup;
use crate::ibc_tests_setup::MultitestAndromeda;
use andromeda_non_fungible_tokens::crowdfund::{
    CampaignConfig, ExecuteMsg as CrowdfundExecuteMsg, InstantiateMsg, SimpleTierOrder, Tier,
    TierMetaData,
};
use andromeda_std::{
    amp::{AndrAddr, Recipient},
    common::{denom::Asset, expiration::Expiry, Milliseconds},
};
use cosmwasm_std::{coins, Addr, Uint128, Uint64};
use cw_multi_test::{AppResponse, ContractWrapper, Executor};

use andromeda_std::{amp::ADO_DB_KEY, os::kernel::ExecuteMsg as KernelExecuteMsg};

pub struct IBCCrowdfundTest {
    pub chain_a: MultitestAndromeda,
    pub chain_b: MultitestAndromeda,
    pub crowdfund_address: Addr,
}

impl IBCCrowdfundTest {
    pub fn new() -> Self {
        let (mut chain_a, chain_b) = MultitestAndromeda::setup_ibc_test();
        let users = MultitestAndromeda::mock_users();

        // Store and instantiate Crowdfund contract on chain A
        let crowdfund_code_id = chain_a.app.store_code(Box::new(ContractWrapper::new(
            andromeda_crowdfund::contract::execute,
            andromeda_crowdfund::contract::instantiate,
            andromeda_crowdfund::contract::query,
        )));

        let cw721_code_id = chain_a.app.store_code(Box::new(ContractWrapper::new(
            andromeda_cw721::contract::execute,
            andromeda_cw721::contract::instantiate,
            andromeda_cw721::contract::query,
        )));

        let cw721_address: Addr = chain_a
            .app
            .instantiate_contract(
                cw721_code_id,
                Addr::unchecked("owner"),
                &andromeda_cw721::contract::InstantiateMsg {
                    name: "Test NFT".to_string(),
                    symbol: "NFT".to_string(),
                    minter: None,
                },
                &[],
                "CW721",
                None,
            )
            .unwrap();

        // Setup campaign config
        let campaign_config = CampaignConfig {
            title: Some("IBC Crowdfund".to_string()),
            description: Some("Test IBC crowdfunding".to_string()),
            banner: None,
            url: None,
            denom: Asset::Cw20Token(AndrAddr::from_string(chain_a.cw20_address.clone())),
            token_address: // cw721 address,
            withdrawal_recipient: Recipient {
                address: AndrAddr::from_string(users[0].clone()),
                msg: None,
                ibc_recovery_address: None,
            },
            soft_cap: Some(Uint128::new(100)),
            hard_cap: Some(Uint128::new(1000)),
        };

        // Setup tiers
        let tiers = vec![Tier {
            level: Uint64::new(1),
            label: "Basic".to_string(),
            price: Uint128::new(50),
            limit: Some(Uint128::new(100)),
            metadata: TierMetaData {
                token_uri: None,
                extension: Default::default(),
            },
        }];

        let crowdfund_address: Addr = chain_a
            .app
            .instantiate_contract(
                crowdfund_code_id,
                Addr::unchecked("owner"),
                &InstantiateMsg {
                    campaign_config: campaign_config,
                    tiers: tiers,
                    kernel_address: chain_a.kernel_address.clone(),
                    owner: None,
                },
                &[],
                "Crowdfund",
                None,
            )
            .unwrap();

        chain_a
            .app
            .execute_contract(
                Addr::unchecked("owner"),
                Addr::unchecked(chain_a.kernel_address.clone()),
                &KernelExecuteMsg::UpsertKeyAddress {
                    key: ADO_DB_KEY.to_string(),
                    value: chain_a.adodb_address.clone(),
                },
                &[],
            )
            .unwrap();

        Self {
            chain_a,
            chain_b,
            crowdfund_address,
        }
    }

    pub fn deposit_from_chain_b(&mut self, amount: u128) -> AppResponse {
        println!("Crowdfund address: {}", &self.crowdfund_address);

        // Create IBC transfer from chain B to crowdfund on chain A
        let msg = CrowdfundExecuteMsg::PurchaseTiers {
            orders: vec![SimpleTierOrder {
                level: Uint64::new(1),
                amount: Uint128::new(1),
            }],
        };

        let res = self
            .chain_b
            .app
            .execute_contract(
                Addr::unchecked("user"),
                Addr::unchecked(&self.crowdfund_address),
                &msg,
                &coins(amount, "utoken"),
            )
            .unwrap();

        println!("Deposit response: {:?}", &res);
        res
    }
}

#[test]
fn test_ibc_crowdfund_deposit() {
    let mut test: IBCCrowdfundTest = IBCCrowdfundTest::new();

    // Start campaign
    let resposne = test.chain_a.app.execute_contract(
        Addr::unchecked("owner"),
        Addr::unchecked(&test.crowdfund_address),
        &CrowdfundExecuteMsg::StartCampaign {
            start_time: None,
            end_time: Expiry::FromNow(Milliseconds(100000)),
            presale: None,
        },
        &[],
    );

    println!("Campaign start response: {:?}", resposne.unwrap());
}
