use andromeda_std::ado_base::InstantiateMsg;
use andromeda_std::ado_contract::ADOContract;

use andromeda_std::testing::mock_querier::MockAndromedaQuerier;
use cosmwasm_std::testing::mock_info;
use cosmwasm_std::{
    from_json,
    testing::{mock_env, MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR},
    to_json_binary, Binary, Coin, ContractResult, OwnedDeps, Querier, QuerierResult, QueryRequest,
    SystemError, SystemResult, WasmQuery,
};

use cw721::{Cw721QueryMsg, OwnerOfResponse, TokensResponse};

pub use andromeda_std::testing::mock_querier::MOCK_KERNEL_CONTRACT;

pub const MOCK_TOKEN_CONTRACT: &str = "token_contract";
pub const MOCK_UNCLAIMED_TOKEN: &str = "unclaimed_token";
pub const MOCK_TOKEN_ADDR: &str = "token_addr";
pub const MOCK_TOKEN_OWNER: &str = "owner";
pub const MOCK_TOKENS_FOR_SALE: &[&str] = &[
    "token1", "token2", "token3", "token4", "token5", "token6", "token7",
];

pub const MOCK_CONDITIONS_MET_CONTRACT: &str = "conditions_met";
pub const MOCK_CONDITIONS_NOT_MET_CONTRACT: &str = "conditions_not_met";

/// Alternative to `cosmwasm_std::testing::mock_dependencies` that allows us to respond to custom queries.
///
/// Automatically assigns a kernel address as MOCK_KERNEL_CONTRACT.
pub fn mock_dependencies_custom(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]));
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
            mock_info("sender", &[]),
            InstantiateMsg {
                ado_type: "crowdfund".to_string(),
                ado_version: "test".to_string(),
                operators: None,
                kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
                owner: None,
            },
        )
        .unwrap();
    deps
}

pub struct WasmMockQuerier {
    pub base: MockQuerier,
    pub contract_address: String,
    pub tokens_left_to_burn: usize,
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<cosmwasm_std::Empty> = match from_json(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {e}"),
                    request: bin_request.into(),
                })
            }
        };
        self.handle_query(&request)
    }
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<cosmwasm_std::Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                match contract_addr.as_str() {
                    MOCK_TOKEN_ADDR => self.handle_token_query(msg),
                    MOCK_TOKEN_CONTRACT => self.handle_token_query(msg),
                    _ => MockAndromedaQuerier::default().handle_query(&self.base, request),
                }
            }
            _ => MockAndromedaQuerier::default().handle_query(&self.base, request),
        }
    }

    fn handle_token_query(&self, msg: &Binary) -> QuerierResult {
        match from_json(msg).unwrap() {
            Cw721QueryMsg::Tokens { owner, .. } => {
                let res = if owner == MOCK_CONDITIONS_MET_CONTRACT
                    || owner == MOCK_CONDITIONS_NOT_MET_CONTRACT
                {
                    TokensResponse {
                        tokens: MOCK_TOKENS_FOR_SALE
                            [MOCK_TOKENS_FOR_SALE.len() - self.tokens_left_to_burn..]
                            .iter()
                            .copied()
                            .map(String::from)
                            .collect(),
                    }
                } else {
                    TokensResponse {
                        tokens: MOCK_TOKENS_FOR_SALE
                            .iter()
                            .copied()
                            .map(String::from)
                            .collect(),
                    }
                };

                SystemResult::Ok(ContractResult::Ok(to_json_binary(&res).unwrap()))
            }
            Cw721QueryMsg::OwnerOf { token_id, .. } => {
                let res = if token_id == MOCK_UNCLAIMED_TOKEN {
                    OwnerOfResponse {
                        owner: mock_env().contract.address.to_string(),
                        approvals: vec![],
                    }
                } else {
                    OwnerOfResponse {
                        owner: MOCK_TOKEN_OWNER.to_owned(),
                        approvals: vec![],
                    }
                };
                SystemResult::Ok(ContractResult::Ok(to_json_binary(&res).unwrap()))
            }

            _ => panic!("Unsupported Query"),
        }
    }

    pub fn new(base: MockQuerier) -> Self {
        WasmMockQuerier {
            base,
            contract_address: mock_env().contract.address.to_string(),
            tokens_left_to_burn: 2,
        }
    }
}
