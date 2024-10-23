use crate::{
    ado_base::AndromedaQuery,
    ado_contract::ADOContract,
    amp::{ADO_DB_KEY, ECONOMICS_KEY, IBC_REGISTRY_KEY, OSMOSIS_ROUTER_KEY, VFS_KEY},
    os::{
        adodb::{ActionFee, QueryMsg as ADODBQueryMsg},
        ibc_registry::{DenomInfo, QueryMsg as IBCRegistryQueryMsg},
        kernel::{ChannelInfo, QueryMsg as KernelQueryMsg},
        vfs::QueryMsg as VFSQueryMsg,
        IBC_VERSION, TRANSFER_PORT,
    },
};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    from_json,
    testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR},
    to_json_binary, Addr, Binary, CodeInfoResponse, Coin, ContractInfoResponse, ContractResult,
    HexBinary, IbcChannel, IbcEndpoint, IbcOrder, OwnedDeps, Querier, QuerierResult, QueryRequest,
    SubMsg, SystemError, SystemResult, Uint128, WasmQuery,
};
#[cfg(feature = "primitive")]
use cosmwasm_std::{Decimal, Uint128};
use cw20::{BalanceResponse, Cw20QueryMsg, TokenInfoResponse};

/// Mock CW20 Contract Address
pub const MOCK_CW20_CONTRACT: &str = "cw20_contract";
/// Mock Anchor Contract Address
pub const MOCK_ANCHOR_CONTRACT: &str = "anchor_contract";
/// Mock App Contract Address
pub const MOCK_APP_CONTRACT: &str = "app_contract";
/// Mock System ADO Address
pub const MOCK_SYSTEM_CONTRACT: &str = "system_contract";
/// Mock Primitive Contract Address
pub const MOCK_PRIMITIVE_CONTRACT: &str = "primitive_contract";
/// Mock Kernel Contract Address
pub const MOCK_KERNEL_CONTRACT: &str = "kernel_contract";
/// Mock Kernel Contract Address on foreign chain
pub const MOCK_FAKE_KERNEL_CONTRACT: &str = "fake_kernel_contract";
/// Mock VFS Contract Address
pub const MOCK_VFS_CONTRACT: &str = "vfs_contract";
/// Mock ADODB Contract Address
pub const MOCK_ADODB_CONTRACT: &str = "adodb_contract";
/// Mock IBC Registry Contract Address
pub const MOCK_IBC_REGISTRY_CONTRACT: &str = "ibc_registry_contract";
// Mock ADO Publisher
pub const MOCK_ADO_PUBLISHER: &str = "ado_publisher";
// Mock Osmosis Router
pub const MOCK_OSMOSIS_ROUTER_CONTRACT: &str = "osmosis_router";
// Mock Economics Contract
pub const MOCK_ECONOMICS_CONTRACT: &str = "economics_contract";

/// Mock Rates Contract Address
pub const MOCK_RATES_CONTRACT: &str = "rates_contract";
/// Mock Address List Contract Address
pub const MOCK_ADDRESS_LIST_CONTRACT: &str = "address_list_contract";

/// An invalid contract address
pub const INVALID_CONTRACT: &str = "invalid_contract";
/// An invalid VFS Path
pub const FAKE_VFS_PATH: &str = "/f";
/// An invalid ADODB Key
pub const FAKE_ADODB_KEY: &str = "fake_adodb_key";
/// A valid action
pub const MOCK_ACTION: &str = "action";
pub const UNWHITELISTED_ADDRESS: &str = "unwhitelisted_address";
pub const RATES_EXCLUDED_ADDRESS: &str = "rates_excluded_address";

pub const MOCK_CHECKSUM: &str = "9af782a3a1bcbcd22dbb6a45c751551d9af782a3a1bcbcd22dbb6a45c751551d";

pub const MOCK_WALLET: &str = "mock_wallet";

pub const MOCK_UANDR: &str = "mock_uandr";

pub const MOCK_OSMO_IBC_DENOM_ON_ANDR: &str =
    "ibc/ed07a3391a112b175915cd8faf43a2da8e4790ede12566649d0c2f97716b8518";
pub const MOCK_OSMO_IBC_PATH: &str = "transfer/channel-0";
pub const MOCK_ANDR_TO_OSMO_IBC_CHANNEL: &str = "channel-0";
pub const MOCK_UANDR_IBC_DENOM_ON_OSMO: &str =
    "ibc/8084e3357a8a56809fff009114e8b62b88b6f931ef7de1db44751c1ab42d51e9";
pub const MOCK_OSMO_TO_ANDR_IBC_CHANNEL: &str = "channel-1";
pub const MOCK_UANDR_NATIVE_DENOM: &str = "uandr";
pub const MOCK_OSMO_NATIVE_DENOM: &str = "uosmo";

pub struct WasmMockQuerier {
    pub base: MockQuerier,
}

/// Alternative to `cosmwasm_std::testing::mock_dependencies` that allows us to respond to custom queries.
///
/// Automatically assigns a kernel address as MOCK_KERNEL_CONTRACT.
pub fn mock_dependencies_custom(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let mut custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]));

    // Add IBC Channels to mock querier
    custom_querier.base.update_ibc(
        TRANSFER_PORT,
        &[IbcChannel::new(
            IbcEndpoint {
                port_id: TRANSFER_PORT.to_string(),
                channel_id: MOCK_ANDR_TO_OSMO_IBC_CHANNEL.to_string(),
            },
            IbcEndpoint {
                port_id: TRANSFER_PORT.to_string(),
                channel_id: MOCK_OSMO_TO_ANDR_IBC_CHANNEL.to_string(),
            },
            IbcOrder::Unordered,
            IBC_VERSION.to_string(),
            String::from("connection-0"),
        )],
    );
    let storage = MockStorage::default();
    let mut deps = OwnedDeps {
        storage,
        api: MockApi::default(),
        querier: custom_querier,
        custom_query_type: std::marker::PhantomData,
    };
    ADOContract::default()
        .kernel_address
        .save(
            deps.as_mut().storage,
            &Addr::unchecked(MOCK_KERNEL_CONTRACT),
        )
        .unwrap();
    deps
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
    /// A custom query handler that provides custom handling for the mock contract addresses provided in this crate.
    ///
    /// Each contract address has its own handler within the Querier and is called when the contract address is set as such.
    ///
    /// A custom response is added for `cosmwasm_std::ContractInfo` queries that returns a code id of 2 for `INVALID_CONTRACT` and 1 for all other addresses.
    ///
    /// Any other addresses are handled by the default querier.
    pub fn handle_query(&self, request: &QueryRequest<cosmwasm_std::Empty>) -> QuerierResult {
        MockAndromedaQuerier::default().handle_query(&self.base, request)
    }

    pub fn new(base: MockQuerier) -> Self {
        WasmMockQuerier { base }
    }
}

// NOTE: It's impossible to construct a non_exhaustive struct from another another crate, so I copied the struct
// https://rust-lang.github.io/rfcs/2008-non-exhaustive.html#functional-record-updates
#[cw_serde(
    Serialize,
    Deserialize,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    JsonSchema
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub struct SupplyResponse {
    /// Always returns a Coin with the requested denom.
    /// This will be of zero amount if the denom does not exist.
    pub amount: Coin,
}

#[derive(Default)]
pub struct MockAndromedaQuerier {}

impl MockAndromedaQuerier {
    pub fn handle_query(
        self,
        querier: &MockQuerier,
        request: &QueryRequest<cosmwasm_std::Empty>,
    ) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                match contract_addr.as_str() {
                    MOCK_CW20_CONTRACT => self.handle_cw20_query(msg),
                    MOCK_APP_CONTRACT => self.handle_app_query(msg),
                    #[cfg(feature = "primitive")]
                    MOCK_PRIMITIVE_CONTRACT => self.handle_primitive_query(msg),
                    MOCK_KERNEL_CONTRACT => self.handle_kernel_query(msg),
                    MOCK_VFS_CONTRACT => self.handle_vfs_query(msg),
                    MOCK_ADODB_CONTRACT => self.handle_adodb_query(msg),
                    MOCK_IBC_REGISTRY_CONTRACT => self.handle_ibc_registry_query(msg),
                    MOCK_UANDR => self.handle_cw20_query(msg),

                    // MOCK_ADDRESS_LIST_CONTRACT => self.handle_address_list_query(msg),
                    _ => match from_json::<AndromedaQuery>(msg) {
                        Ok(msg) => self.handle_ado_query(msg),
                        _ => SystemResult::Err(SystemError::InvalidRequest {
                            error: "Unsupported query".to_string(),
                            request: to_json_binary(&request).unwrap(),
                        }),
                    },
                }
            }
            QueryRequest::Wasm(WasmQuery::Raw { contract_addr, key }) => {
                match contract_addr.as_str() {
                    // MOCK_APP_CONTRACT => self.handle_app_raw_query(key),
                    MOCK_KERNEL_CONTRACT => self.handle_kernel_raw_query(key, false),
                    MOCK_FAKE_KERNEL_CONTRACT => self.handle_kernel_raw_query(key, true),
                    MOCK_ADODB_CONTRACT => self.handle_adodb_raw_query(key),
                    MOCK_CW20_CONTRACT => self.handle_cw20_owner_query(key),
                    MOCK_ANCHOR_CONTRACT => self.handle_anchor_owner_query(key),
                    MOCK_UANDR => self.handle_cw20_owner_query(key),

                    _ => self.handle_ado_raw_query(key, &Addr::unchecked(contract_addr)),
                }
            }
            // Defaults to code ID 1, returns 2 for `INVALID_CONTRACT` which is considered an invalid ADODB code id
            QueryRequest::Wasm(WasmQuery::ContractInfo { contract_addr }) => {
                if contract_addr == MOCK_WALLET {
                    return SystemResult::Ok(ContractResult::Err(
                        "Not a valid contract".to_string(),
                    ));
                }
                let mut resp = ContractInfoResponse::default();
                resp.code_id = match contract_addr.as_str() {
                    MOCK_SYSTEM_CONTRACT => 4,
                    MOCK_APP_CONTRACT => 3,
                    INVALID_CONTRACT => 2,
                    _ => 1,
                };
                SystemResult::Ok(ContractResult::Ok(to_json_binary(&resp).unwrap()))
            }
            QueryRequest::Wasm(WasmQuery::CodeInfo { code_id }) => {
                if *code_id == 2u64 {
                    return SystemResult::Ok(ContractResult::Err("Invalid Code ID".to_string()));
                }
                let mut resp = CodeInfoResponse::default();
                resp.checksum = HexBinary::from_hex(MOCK_CHECKSUM).unwrap();
                SystemResult::Ok(ContractResult::Ok(to_json_binary(&resp).unwrap()))
            }
            _ => querier.handle_query(request),
        }
    }

    fn handle_cw20_owner_query(&self, _msg: &Binary) -> QuerierResult {
        SystemResult::Ok(ContractResult::Ok(
            to_json_binary("cosmos2contract").unwrap(),
        ))
    }

    fn handle_anchor_owner_query(&self, _msg: &Binary) -> QuerierResult {
        SystemResult::Ok(ContractResult::Ok(
            to_json_binary("cosmos2contract").unwrap(),
        ))
    }

    /// Handles all kernel queries.
    ///
    /// Returns the appropriate `MOCK_CONTRACT_*` address for the given key in the case of a `KeyAddress` query.
    ///
    /// Returns `true` for `VerifyAddress` for any address excluding `INVALID_CONTRACT`.
    fn handle_kernel_query(&self, msg: &Binary) -> QuerierResult {
        match from_json(msg).unwrap() {
            KernelQueryMsg::KeyAddress { key } => match key.as_str() {
                VFS_KEY => SystemResult::Ok(ContractResult::Ok(
                    to_json_binary(&MOCK_VFS_CONTRACT).unwrap(),
                )),
                ADO_DB_KEY => SystemResult::Ok(ContractResult::Ok(
                    to_json_binary(&MOCK_ADODB_CONTRACT).unwrap(),
                )),
                IBC_REGISTRY_KEY => SystemResult::Ok(ContractResult::Ok(
                    to_json_binary(&MOCK_IBC_REGISTRY_CONTRACT).unwrap(),
                )),
                &_ => SystemResult::Ok(ContractResult::Err("Invalid Key".to_string())),
            },
            KernelQueryMsg::VerifyAddress { address } => match address.as_str() {
                INVALID_CONTRACT => {
                    SystemResult::Ok(ContractResult::Err("Invalid Address".to_string()))
                }
                _ => SystemResult::Ok(ContractResult::Ok(to_json_binary(&true).unwrap())),
            },
            _ => SystemResult::Ok(ContractResult::Err("Not implemented".to_string())),
        }
    }

    /// Handles all VFS queries.
    ///
    /// Returns the path provided for `ResolvePath` queries, or an error for`FAKE_PATH`.
    fn handle_vfs_query(&self, msg: &Binary) -> QuerierResult {
        match from_json(msg).unwrap() {
            VFSQueryMsg::ResolvePath { path } => match path.as_str() {
                FAKE_VFS_PATH => SystemResult::Ok(ContractResult::Err("Invalid Path".to_string())),
                _ => SystemResult::Ok(ContractResult::Ok(to_json_binary(&path).unwrap())),
            },
            VFSQueryMsg::ResolveSymlink { path } => match path.as_str() {
                FAKE_VFS_PATH => SystemResult::Ok(ContractResult::Err("Invalid Path".to_string())),
                _ => SystemResult::Ok(ContractResult::Ok(to_json_binary(&path).unwrap())),
            },
            VFSQueryMsg::SubDir { path, .. } => match path.as_str() {
                FAKE_VFS_PATH => SystemResult::Ok(ContractResult::Err("Invalid Path".to_string())),
                _ => SystemResult::Ok(ContractResult::Ok(to_json_binary(&path).unwrap())),
            },
            VFSQueryMsg::Paths { addr } => {
                SystemResult::Ok(ContractResult::Ok(to_json_binary(&addr).unwrap()))
            }
            VFSQueryMsg::GetUsername { address } => {
                SystemResult::Ok(ContractResult::Ok(to_json_binary(&address).unwrap()))
            }
            VFSQueryMsg::GetLibrary { address } => {
                SystemResult::Ok(ContractResult::Ok(to_json_binary(&address).unwrap()))
            }
            _ => todo!(),
        }
    }

    /// Handles all App queries.
    ///
    /// Returns `"actual_address"` for `Get` queries.
    fn handle_app_query(&self, _msg: &Binary) -> QuerierResult {
        // match from_json(msg).unwrap() {
        // match from_json(msg).unwrap() {
        //     _ => SystemResult::Ok(ContractResult::Err("Error".to_string())),
        // }
        todo!()
    }

    /// Handles all ADODB queries.
    ///
    /// Returns `"ADOType"` for `ADOType` queries with code ID 1 and an error otherwise.
    ///
    /// Returns an error for `CodeId` queries with key `FAKE_ADODB_KEY` and 1 otherwise.
    fn handle_adodb_query(&self, msg: &Binary) -> QuerierResult {
        match from_json(msg).unwrap() {
            ADODBQueryMsg::ADOType { code_id } => match code_id {
                3 => SystemResult::Ok(ContractResult::Ok(to_json_binary(&"app-contract").unwrap())),
                1 => SystemResult::Ok(ContractResult::Ok(to_json_binary(&"ADOType").unwrap())),
                _ => SystemResult::Ok(ContractResult::Err("Invalid Code ID".to_string())),
            },
            ADODBQueryMsg::CodeId { key } => match key.as_str() {
                FAKE_ADODB_KEY => SystemResult::Ok(ContractResult::Err("Invalid Key".to_string())),
                _ => SystemResult::Ok(ContractResult::Ok(to_json_binary(&1).unwrap())),
            },
            _ => SystemResult::Ok(ContractResult::Err("Not implemented".to_string())),
        }
    }

    /// Handles all IBC Registry queries.
    ///
    /// Returns the denom info for `MOCK_OSMO_IBC_DENOM_ON_ANDR` and an error otherwise.
    ///
    /// Returns an error for any other denom.
    fn handle_ibc_registry_query(&self, msg: &Binary) -> QuerierResult {
        match from_json(msg).unwrap() {
            IBCRegistryQueryMsg::DenomInfo { denom } => match denom.as_str() {
                MOCK_OSMO_IBC_DENOM_ON_ANDR => SystemResult::Ok(ContractResult::Ok(
                    to_json_binary(&DenomInfo {
                        path: MOCK_OSMO_IBC_PATH.to_string(),
                        base_denom: MOCK_OSMO_NATIVE_DENOM.to_string(),
                    })
                    .unwrap(),
                )),
                _ => SystemResult::Ok(ContractResult::Err("Invalid Denom".to_string())),
            },
            _ => SystemResult::Ok(ContractResult::Err("Not implemented".to_string())),
        }
    }

    #[cfg(feature = "primitive")]
    /// Handles all primitive queries.
    ///
    /// Returns a default value for each primitive type.
    fn handle_primitive_query(&self, msg: &Binary) -> QuerierResult {
        match from_json(msg).unwrap() {
            QueryMsg::AndrQuery(AndromedaQuery::Get(data)) => {
                let res = match data {
                    None => GetValueResponse {
                        key: "default".to_string(),
                        value: Primitive::Decimal(Decimal::zero()),
                    },
                    Some(data) => {
                        let key: String = from_json(&data).unwrap();
                        let key: String = from_json(&data).unwrap();
                        match key.as_str() {
                            "String" => GetValueResponse {
                                key,
                                value: Primitive::String("Value".to_string()),
                            },
                            "Uint128" => GetValueResponse {
                                key,
                                value: Primitive::Uint128(Uint128::new(10)),
                            },
                            "Decimal" => GetValueResponse {
                                key,
                                value: Primitive::Decimal(Decimal::percent(1)),
                            },
                            "Coin" => GetValueResponse {
                                key,
                                value: Primitive::Coin(Coin::new(100, "uusd")),
                            },
                            "Bool" => GetValueResponse {
                                key,
                                value: Primitive::Bool(true),
                            },
                            "Vec" => GetValueResponse {
                                key,
                                value: Primitive::Vec(vec![Primitive::from("String".to_string())]),
                            },
                            _ => {
                                return SystemResult::Ok(ContractResult::Err(
                                    "Not Found".to_string(),
                                ))
                            }
                        }
                    }
                };

                SystemResult::Ok(ContractResult::Ok(to_json_binary(&res).unwrap()))
            }
            _ => panic!("Unsupported Query"),
        }
    }

    /// Handles all CW20 queries.
    ///
    /// Returns a balance of 10 for any `Balance` query.
    fn handle_cw20_query(&self, msg: &Binary) -> QuerierResult {
        match from_json(msg).unwrap() {
            Cw20QueryMsg::Balance { .. } => {
                let balance_response = BalanceResponse {
                    balance: 10u128.into(),
                };
                SystemResult::Ok(ContractResult::Ok(
                    to_json_binary(&balance_response).unwrap(),
                ))
            }
            Cw20QueryMsg::TokenInfo {} => {
                let token_info_response = TokenInfoResponse {
                    name: "valid-cw20".into(),
                    symbol: "VCW".to_string(),
                    decimals: 2,
                    total_supply: Uint128::new(10_000_000),
                };
                SystemResult::Ok(ContractResult::Ok(
                    to_json_binary(&token_info_response).unwrap(),
                ))
            }
            _ => panic!("Unsupported Query"),
        }
    }

    pub fn handle_ado_raw_query(&self, key: &Binary, contract_addr: &Addr) -> QuerierResult {
        let key_vec = key.as_slice();
        let key_str = String::from_utf8(key_vec.to_vec()).unwrap();

        if key_str.contains("owner") {
            return SystemResult::Ok(ContractResult::Ok(
                to_json_binary(&Addr::unchecked("owner".to_string())).unwrap(),
            ));
        }

        panic!("Unsupported query for contract: {contract_addr}")
    }

    pub fn handle_kernel_raw_query(&self, key: &Binary, fake: bool) -> QuerierResult {
        let key_vec = key.as_slice();
        let key_str = String::from_utf8(key_vec.to_vec()).unwrap();

        if key_str.contains("kernel_addresses") {
            let split = key_str.split("kernel_addresses");
            let key = split.last();
            if let Some(key) = key {
                match key {
                    VFS_KEY => SystemResult::Ok(ContractResult::Ok(
                        to_json_binary(&MOCK_VFS_CONTRACT.to_string()).unwrap(),
                    )),
                    ADO_DB_KEY => SystemResult::Ok(ContractResult::Ok(
                        to_json_binary(&MOCK_ADODB_CONTRACT.to_string()).unwrap(),
                    )),
                    OSMOSIS_ROUTER_KEY => SystemResult::Ok(ContractResult::Ok(
                        to_json_binary(&MOCK_OSMOSIS_ROUTER_CONTRACT.to_string()).unwrap(),
                    )),
                    ECONOMICS_KEY => SystemResult::Ok(ContractResult::Ok(
                        to_json_binary(&MOCK_ECONOMICS_CONTRACT.to_string()).unwrap(),
                    )),
                    _ => panic!("Invalid Kernel Address Key"),
                }
            } else {
                panic!("Invalid Kernel Address Raw Query")
            }
        } else if key_str.contains("curr_chain") {
            let res = if fake {
                "fake_chain".to_string()
            } else {
                "andromeda".to_string()
            };
            SystemResult::Ok(ContractResult::Ok(to_json_binary(&res).unwrap()))
        } else if key_str.contains("channel") {
            SystemResult::Ok(ContractResult::Ok(
                to_json_binary(&ChannelInfo {
                    kernel_address: "kernel".to_string(),
                    ics20_channel_id: Some("1".to_string()),
                    direct_channel_id: Some("2".to_string()),
                    supported_modules: vec![],
                })
                .unwrap(),
            ))
        } else {
            panic!("Invalid Kernel Raw Query")
        }
    }

    pub fn handle_adodb_raw_query(&self, key: &Binary) -> QuerierResult {
        let key_vec = key.as_slice();
        let key_str = String::from_utf8(key_vec.to_vec()).unwrap();

        if key_str.contains("code_id") {
            let split = key_str.split("code_id");
            let key = split.last();
            if let Some(key) = key {
                match key {
                    FAKE_ADODB_KEY => {
                        SystemResult::Ok(ContractResult::Err("Invalid Key".to_string()))
                    }
                    _ => SystemResult::Ok(ContractResult::Ok(to_json_binary(&1).unwrap())),
                }
            } else {
                panic!("Invalid ADODB Raw Query")
            }
        } else if key_str.contains("action_fees") {
            let split = key_str.split("action_fees");
            let key = split.last();
            match key {
                Some(key) => {
                    if key.contains("ADOTypeaction") {
                        SystemResult::Ok(ContractResult::Ok(
                            to_json_binary(&ActionFee::new(
                                MOCK_ACTION.to_string(),
                                "native:uusd".to_string(),
                                Uint128::from(10u128),
                            ))
                            .unwrap(),
                        ))
                    } else {
                        SystemResult::Ok(ContractResult::Err("Invalid Key".to_string()))
                    }
                }
                None => SystemResult::Ok(ContractResult::Err("Invalid Key".to_string())),
            }
        } else if key_str.contains("ado_type") {
            let split = key_str.split("ado_type");
            let key = split.last();
            // let app_contract_key = String::from_utf8(3u64.to_be_bytes().to_vec()).unwrap();
            // let generic_contract_key = String::from_utf8(1u64.to_be_bytes().to_vec()).unwrap();
            if let Some(key) = key {
                if key == "4" {
                    SystemResult::Ok(ContractResult::Ok(
                        to_json_binary("system-contract").unwrap(),
                    ))
                } else if key == "3" {
                    SystemResult::Ok(ContractResult::Ok(to_json_binary("app-contract").unwrap()))
                } else if key == "1" {
                    SystemResult::Ok(ContractResult::Ok(to_json_binary("ADOType").unwrap()))
                } else {
                    SystemResult::Ok(ContractResult::Ok(Binary::default()))
                }
            } else {
                SystemResult::Ok(ContractResult::Err("Invalid Key".to_string()))
            }
        } else if key_str.contains("publisher") {
            let split = key_str.split("ado_type");
            let key = split.last();
            match key {
                Some(key) => match key {
                    FAKE_ADODB_KEY => {
                        SystemResult::Ok(ContractResult::Err("Invalid Key".to_string()))
                    }
                    _ => SystemResult::Ok(ContractResult::Ok(
                        to_json_binary(MOCK_ADO_PUBLISHER).unwrap(),
                    )),
                },
                None => SystemResult::Ok(ContractResult::Err("Invalid Key".to_string())),
            }
        } else {
            panic!("Invalid ADODB Raw Query")
        }
    }

    pub fn handle_ado_query(&self, msg: AndromedaQuery) -> QuerierResult {
        match msg {
            AndromedaQuery::AppContract {} => SystemResult::Ok(ContractResult::Ok(
                to_json_binary(&MOCK_APP_CONTRACT.to_string()).unwrap(),
            )),
            _ => panic!("Unsupported ADO query"),
        }
    }
}

pub fn calculate_mock_rates_response() -> (Vec<SubMsg>, Vec<Coin>) {
    todo!("Implement after readding rates contract");
}
