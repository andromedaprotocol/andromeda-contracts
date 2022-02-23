use crate::{
    address_list::{IncludesAddressResponse, QueryMsg as AddressListQueryMsg},
    auction::{AuctionStateResponse, QueryMsg as AuctionQueryMsg},
    communication::{
        hooks::{AndromedaHook, OnFundsTransferResponse},
        AndromedaQuery,
    },
    cw721::{
        MetadataAttribute, MetadataType, QueryMsg as Cw721QueryMsg, TokenExtension, TokenMetadata,
    },
    factory::{CodeIdResponse, QueryMsg as FactoryQueryMsg},
    ownership::ContractOwnerResponse,
    primitive::{GetValueResponse, Primitive, QueryMsg as PrimitiveQueryMsg},
    rates::{Funds, QueryMsg as RatesQueryMsg},
    receipt::{generate_receipt_message, QueryMsg as ReceiptQueryMsg},
};
use cosmwasm_std::{
    coin, from_binary, from_slice,
    testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR},
    to_binary, BankMsg, Binary, Coin, ContractResult, CosmosMsg, Decimal, Event, OwnedDeps,
    Querier, QuerierResult, QueryRequest, Response, SubMsg, SystemError, SystemResult, Timestamp,
    Uint128, WasmMsg, WasmQuery,
};
use cw20::{BalanceResponse, Cw20Coin, Cw20ExecuteMsg, Cw20QueryMsg};

use cw721::{Expiration, NftInfoResponse};
use std::collections::HashMap;
use terra_cosmwasm::{TaxCapResponse, TaxRateResponse, TerraQuery, TerraQueryWrapper, TerraRoute};

pub const MOCK_FACTORY_CONTRACT: &str = "factory_contract";
pub const MOCK_CW721_CONTRACT: &str = "cw721_contract";
pub const MOCK_AUCTION_CONTRACT: &str = "auction_contract";
pub const MOCK_PRIMITIVE_CONTRACT: &str = "primitive_contract";
pub const MOCK_CW20_CONTRACT: &str = "cw20_contract";
pub const MOCK_RATES_CONTRACT: &str = "rates_contract";
pub const MOCK_ADDRESSLIST_CONTRACT: &str = "addresslist_contract";
pub const MOCK_RECEIPT_CONTRACT: &str = "receipt_contract";

pub const MOCK_TOKEN_IN_AUCTION: &str = "token1";

pub fn mock_dependencies_custom(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]));

    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: custom_querier,
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier<TerraQueryWrapper>,
    tax_querier: TaxQuerier,
}

#[derive(Clone, Default)]
pub struct TaxQuerier {
    rate: Decimal,
    // this lets us iterate over all pairs that match the first string
    caps: HashMap<String, Uint128>,
}

impl TaxQuerier {
    pub fn new(rate: Decimal, caps: &[(&String, &Uint128)]) -> Self {
        TaxQuerier {
            rate,
            caps: caps_to_map(caps),
        }
    }
}

fn caps_to_map(caps: &[(&String, &Uint128)]) -> HashMap<String, Uint128> {
    let mut owner_map: HashMap<String, Uint128> = HashMap::new();
    for (denom, cap) in caps.iter() {
        owner_map.insert(denom.to_string(), **cap);
    }
    owner_map
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<TerraQueryWrapper> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };
        self.handle_query(&request)
    }
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<TerraQueryWrapper>) -> QuerierResult {
        match &request {
            QueryRequest::Custom(TerraQueryWrapper { route, query_data }) => {
                if &TerraRoute::Treasury == route {
                    match query_data {
                        TerraQuery::TaxRate {} => {
                            let res = TaxRateResponse {
                                rate: self.tax_querier.rate,
                            };
                            SystemResult::Ok(ContractResult::from(to_binary(&res)))
                        }
                        TerraQuery::TaxCap { denom } => {
                            let cap = self
                                .tax_querier
                                .caps
                                .get(denom)
                                .copied()
                                .unwrap_or_default();
                            let res = TaxCapResponse { cap };
                            SystemResult::Ok(ContractResult::from(to_binary(&res)))
                        }
                        _ => panic!("DO NOT ENTER HERE"),
                    }
                } else {
                    panic!("DO NOT ENTER HERE")
                }
            }
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                match contract_addr.as_str() {
                    "addresslist_contract_address1" => {
                        let msg_response = IncludesAddressResponse { included: true };
                        SystemResult::Ok(ContractResult::Ok(to_binary(&msg_response).unwrap()))
                    }
                    "factory_address" => {
                        let msg_response = ContractOwnerResponse {
                            owner: String::from("creator"),
                        };
                        SystemResult::Ok(ContractResult::Ok(to_binary(&msg_response).unwrap()))
                    }
                    MOCK_CW20_CONTRACT => self.handle_cw20_query(msg),
                    MOCK_CW721_CONTRACT => self.handle_cw721_query(msg),
                    MOCK_PRIMITIVE_CONTRACT => self.handle_primitive_query(msg),
                    MOCK_RATES_CONTRACT => self.handle_rates_query(msg),
                    MOCK_ADDRESSLIST_CONTRACT => self.handle_addresslist_query(msg),
                    MOCK_RECEIPT_CONTRACT => self.handle_receipt_query(msg),
                    MOCK_AUCTION_CONTRACT => self.handle_auction_query(msg),
                    MOCK_FACTORY_CONTRACT => self.handle_factory_query(msg),
                    _ => {
                        let msg_response = IncludesAddressResponse { included: false };
                        SystemResult::Ok(ContractResult::Ok(to_binary(&msg_response).unwrap()))
                    }
                }
            }
            _ => self.base.handle_query(request),
        }
    }

    fn handle_rates_query(&self, msg: &Binary) -> QuerierResult {
        match from_binary(msg).unwrap() {
            RatesQueryMsg::AndrHook(hook_msg) => match hook_msg {
                AndromedaHook::OnFundsTransfer {
                    sender: _,
                    payload: _,
                    amount,
                } => {
                    // Hardcodes a percent rate of 10%.
                    let (new_funds, msg): (Funds, SubMsg) = match amount {
                        Funds::Cw20(ref coin) => (
                            Funds::Cw20(Cw20Coin {
                                amount: coin.amount.multiply_ratio(90u128, 100u128),
                                address: coin.address.clone(),
                            }),
                            SubMsg::new(WasmMsg::Execute {
                                contract_addr: MOCK_CW20_CONTRACT.into(),
                                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                                    recipient: "rates_recipient".to_string(),
                                    amount: coin.amount.multiply_ratio(10u128, 100u128),
                                })
                                .unwrap(),
                                funds: vec![],
                            }),
                        ),
                        Funds::Native(ref coin) => (
                            Funds::Native(Coin {
                                amount: coin.amount.multiply_ratio(90u128, 100u128),
                                denom: coin.denom.clone(),
                            }),
                            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                                to_address: "rates_recipient".into(),
                                amount: vec![Coin {
                                    amount: coin.amount.multiply_ratio(10u128, 100u128),
                                    denom: coin.denom.clone(),
                                }],
                            })),
                        ),
                    };
                    let response = OnFundsTransferResponse {
                        msgs: vec![msg],
                        events: vec![Event::new("Royalty".to_string())],
                        leftover_funds: new_funds,
                    };
                    SystemResult::Ok(ContractResult::Ok(to_binary(&response).unwrap()))
                }
                _ => SystemResult::Ok(ContractResult::Err("UnsupportedOperation".to_string())),
            },

            _ => panic!("Unsupported Query"),
        }
    }

    fn handle_addresslist_query(&self, msg: &Binary) -> QuerierResult {
        match from_binary(msg).unwrap() {
            AddressListQueryMsg::AndrHook(hook_msg) => match hook_msg {
                AndromedaHook::OnExecute { sender, payload: _ } => {
                    let whitelisted_addresses = ["sender", "minter", "purchaser", "creator"];
                    let response: Response = Response::default();
                    if whitelisted_addresses.contains(&sender.as_str()) {
                        SystemResult::Ok(ContractResult::Ok(to_binary(&response).unwrap()))
                    } else {
                        SystemResult::Ok(ContractResult::Err("InvalidAddress".to_string()))
                    }
                }
                _ => SystemResult::Ok(ContractResult::Err("UnsupportedOperation".to_string())),
            },
            _ => panic!("Unsupported Query"),
        }
    }

    fn handle_receipt_query(&self, msg: &Binary) -> QuerierResult {
        match from_binary(msg).unwrap() {
            ReceiptQueryMsg::AndrHook(hook_msg) => match hook_msg {
                AndromedaHook::OnFundsTransfer {
                    sender: _,
                    payload,
                    amount,
                } => {
                    let events: Vec<Event> = from_binary(&payload).unwrap();
                    let receipt_msg =
                        generate_receipt_message(MOCK_RECEIPT_CONTRACT.into(), events).unwrap();
                    let response = OnFundsTransferResponse {
                        msgs: vec![receipt_msg],
                        events: vec![],
                        leftover_funds: amount,
                    };
                    SystemResult::Ok(ContractResult::Ok(to_binary(&response).unwrap()))
                }
                _ => SystemResult::Ok(ContractResult::Err("UnsupportedOperation".to_string())),
            },
            _ => panic!("Unsupported Query"),
        }
    }

    fn handle_factory_query(&self, msg: &Binary) -> QuerierResult {
        match from_binary(msg).unwrap() {
            FactoryQueryMsg::AndrQuery(AndromedaQuery::Get(_)) => {
                let response = CodeIdResponse { code_id: 1 };
                SystemResult::Ok(ContractResult::Ok(to_binary(&response).unwrap()))
            }
            _ => panic!("Unsupported Query"),
        }
    }

    fn handle_cw20_query(&self, msg: &Binary) -> QuerierResult {
        match from_binary(msg).unwrap() {
            Cw20QueryMsg::Balance { .. } => {
                let balance_response = BalanceResponse {
                    balance: 10u128.into(),
                };
                SystemResult::Ok(ContractResult::Ok(to_binary(&balance_response).unwrap()))
            }
            _ => panic!("Unsupported Query"),
        }
    }

    fn handle_cw721_query(&self, msg: &Binary) -> QuerierResult {
        match from_binary(msg).unwrap() {
            Cw721QueryMsg::NftInfo { .. } => {
                let extension = TokenExtension {
                    name: "wrapped_token_id".to_owned(),
                    publisher: "sender".to_owned(),
                    description: None,
                    transfer_agreement: None,
                    metadata: Some(TokenMetadata {
                        data_type: MetadataType::Other,
                        external_url: None,
                        data_url: None,
                        attributes: Some(vec![
                            MetadataAttribute {
                                key: "original_token_id".to_owned(),
                                value: "original_token_id".to_owned(),
                                display_label: None,
                            },
                            MetadataAttribute {
                                key: "original_token_address".to_owned(),
                                value: "original_token_address".to_owned(),
                                display_label: None,
                            },
                        ]),
                    }),
                    archived: false,
                    pricing: None,
                };
                let response = NftInfoResponse {
                    token_uri: None,
                    extension,
                };
                SystemResult::Ok(ContractResult::Ok(to_binary(&response).unwrap()))
            }
            _ => panic!("Unsupported Query"),
        }
    }

    fn handle_primitive_query(&self, msg: &Binary) -> QuerierResult {
        match from_binary(msg).unwrap() {
            PrimitiveQueryMsg::GetValue { name } => {
                let msg_response = match name.clone().unwrap().as_str() {
                    "percent" => GetValueResponse {
                        name: name.unwrap(),
                        value: Primitive::Uint128(1u128.into()),
                    },
                    "flat" => GetValueResponse {
                        name: name.unwrap(),
                        value: Primitive::Coin(coin(1u128, "uusd")),
                    },
                    "flat_cw20" => GetValueResponse {
                        name: name.unwrap(),
                        value: Primitive::Coin(coin(1u128, "address")),
                    },
                    _ => panic!("Unsupported rate name"),
                };
                SystemResult::Ok(ContractResult::Ok(to_binary(&msg_response).unwrap()))
            }
            _ => panic!("Unsupported Query"),
        }
    }

    fn handle_auction_query(&self, msg: &Binary) -> QuerierResult {
        match from_binary(msg).unwrap() {
            AuctionQueryMsg::LatestAuctionState { token_id } => {
                let mut res = AuctionStateResponse {
                    start_time: Expiration::AtTime(Timestamp::from_seconds(100)),
                    end_time: Expiration::AtTime(Timestamp::from_seconds(200)),
                    high_bidder_addr: "address".to_string(),
                    high_bidder_amount: Uint128::from(100u128),
                    auction_id: Uint128::zero(),
                    coin_denom: "uusd".to_string(),
                    claimed: true,
                    whitelist: None,
                };
                if token_id == MOCK_TOKEN_IN_AUCTION {
                    res.claimed = false;
                }
                SystemResult::Ok(ContractResult::Ok(to_binary(&res).unwrap()))
            }
            _ => panic!("Unsupported Query"),
        }
    }

    pub fn new(base: MockQuerier<TerraQueryWrapper>) -> Self {
        WasmMockQuerier {
            base,
            tax_querier: TaxQuerier::default(),
        }
    }

    pub fn with_tax(&mut self, rate: Decimal, caps: &[(&String, &Uint128)]) {
        self.tax_querier = TaxQuerier::new(rate, caps);
    }
}
