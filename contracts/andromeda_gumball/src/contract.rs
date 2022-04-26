use crate::state::{CW721_CONTRACT, LIST, STATE, STATUS};
use ado_base::ADOContract;
use andromeda_protocol::gumball::{LatestRandomResponse, State};
use andromeda_protocol::{
    cw721::{ExecuteMsg as Cw721ExecuteMsg, MintMsg, TokenExtension},
    gumball::{
        ExecuteMsg, InstantiateMsg, NumberOfNFTsResponse, QueryMsg, StateResponse, StatusResponse,
    },
};
use common::{
    ado_base::{recipient::Recipient, InstantiateMsg as BaseInstantiateMsg},
    encode_binary,
    error::ContractError,
    require,
};
use cosmwasm_std::{attr, entry_point, Binary};
use cosmwasm_std::{
    Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, QueryRequest, Response, Uint128, WasmMsg,
    WasmQuery,
};
use cw2::set_contract_version;
const CONTRACT_NAME: &str = "crates.io:andromeda_gumball";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const TERRAND_ADDRESS_TESTNET: &str = "terra1a62jxn3hh54fa5slan4dkd7u6v4nzgz3pjhygm";

pub const MOCK_TOKEN_CONTRACT: &str = "cw721_contract";
pub const MOCK_PRIMITIVE_CONTRACT: &str = "primitive_contract";
pub const MOCK_RATES_CONTRACT: &str = "rates_contract";

pub const GENESIS_TIME: u64 = 1595431050;
pub const PERIOD: u64 = 30;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    CW721_CONTRACT.save(deps.storage, &msg.andromeda_cw721_contract)?;
    // Set initial status to false since there's nothing to buy upon instantiation
    let new_list: Vec<String> = Vec::new();
    LIST.save(deps.storage, &new_list)?;
    STATUS.save(deps.storage, &false)?;
    ADOContract::default().instantiate(
        deps.storage,
        deps.api,
        info,
        BaseInstantiateMsg {
            ado_type: "gumball".to_string(),
            operators: None,
            modules: None,
            primitive_contract: None,
        },
    )
}
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    match msg {
        ExecuteMsg::AndrReceive(msg) => contract.execute(deps, env, info, msg, execute),
        ExecuteMsg::Mint(mint_msg) => execute_mint(deps, env, info, mint_msg),
        ExecuteMsg::Buy {} => execute_buy(deps, env, info),
        ExecuteMsg::SaleDetails {
            price,
            max_amount_per_wallet,
            recipient,
        } => execute_sale_details(deps, env, info, price, max_amount_per_wallet, recipient),
        ExecuteMsg::SwitchStatus {} => execute_switch_status(deps, info),
        // ExecuteMsg::SetContractAddress {
        //     andromeda_cw721_contract,
        // } => execute_switch_contract_address(deps, info, andromeda_cw721_contract),
    }
}
// fn execute_switch_contract_address(
//     deps: DepsMut,
//     info: MessageInfo,
//     msg: AndrAddress,
// ) -> Result<Response, ContractError> {
//     let contract = ADOContract::default();

//     require(
//         contract.is_contract_owner(deps.storage, info.sender.as_str())?,
//         ContractError::Unauthorized {},
//     )?;
//     CW721_CONTRACT.save(deps.storage, &msg)?;
//     Ok(Response::new().add_attribute("action", "set cw721 address"))
// }
fn execute_switch_status(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    let mut status = STATUS.load(deps.storage)?;
    require(
        contract.is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {},
    )?;
    // Automatically switch to opposite status
    if status {
        status = false;
    } else {
        status = true;
    }
    STATUS.save(deps.storage, &status)?;
    Ok(Response::new().add_attribute("action", "Switched Status"))
}
fn execute_sale_details(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    price: Coin,
    max_amount_per_wallet: Option<Uint128>,
    recipient: Recipient,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    let status = STATUS.load(deps.storage)?;
    // Check status, can't change sale details while buying is allowed
    require(!status, ContractError::Refilling {})?;
    // Check authority
    require(
        contract.is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {},
    )?;
    // Check valid amount
    require(
        price.amount > Uint128::from(0_u64),
        ContractError::InvalidZeroAmount {},
    )?;
    // Check valid denomination
    require(
        price.denom == *"uusd",
        ContractError::InvalidFunds {
            msg: "Only uusd is allowed".to_string(),
        },
    )?;
    // Check valid max amount per wallet
    let max_amount_per_wallet = max_amount_per_wallet.unwrap_or_else(|| Uint128::from(1u128));

    require(
        max_amount_per_wallet > Uint128::from(0_u64),
        ContractError::InvalidZeroAmount {},
    )?;
    // This is to prevent cloning price.
    let price_str = price.to_string();

    // Set the state
    let state = State {
        price,
        max_amount_per_wallet,
        recipient: recipient.clone(),
    };
    STATE.save(deps.storage, &state)?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "switch status"),
        attr("price", price_str),
        attr("max_amount_per_wallet", max_amount_per_wallet),
        attr(
            "recipient",
            recipient.get_addr(
                deps.api,
                &deps.querier,
                contract.get_mission_contract(deps.storage)?,
            )?,
        ),
    ]))
}
fn execute_mint(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    mint_msg: Box<MintMsg<TokenExtension>>,
) -> Result<Response, ContractError> {
    let status = STATUS.load(deps.storage)?;
    // Can only mint when in "refill" mode, and that's when status is set to false.
    require(!status, ContractError::NotInRefillMode {})?;
    let contract = ADOContract::default();
    // check authority
    require(
        contract.is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {},
    )?;
    let mut list = LIST.load(deps.storage)?;

    let config = CW721_CONTRACT.load(deps.storage)?;
    // Add to list of NFTs
    list.push(mint_msg.clone().token_id);

    LIST.save(deps.storage, &list).unwrap();
    let mission_contract = contract.get_mission_contract(deps.storage)?;

    let contract_addr = config.get_address(deps.api, &deps.querier, mission_contract)?;

    Ok(Response::new()
        .add_attribute("action", "mint")
        .add_message(WasmMsg::Execute {
            contract_addr,
            msg: encode_binary(&Cw721ExecuteMsg::Mint(mint_msg))?,
            funds: vec![],
        }))
}
fn execute_buy(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let status = STATUS.load(deps.storage)?;
    // check gumball's status
    require(status, ContractError::Refilling {})?;
    let mut list = LIST.load(deps.storage)?;
    let n_of_nfts = list.len();
    // check if we still have any NFTs left
    require(n_of_nfts > 0, ContractError::OutOfNFTs {})?;
    // check if more than one type of coin was sent
    require(
        info.funds.len() == 1,
        ContractError::InvalidFunds {
            msg: "Only one type of coin is required (uusd).".to_string(),
        },
    )?;
    let sent_funds = &info.funds[0];
    // check for correct denomination
    require(
        sent_funds.denom == *"uusd",
        ContractError::InvalidFunds {
            msg: "Only uusd is accepted".to_string(),
        },
    )?;
    let state = STATE.load(deps.storage)?;

    // check for correct amount of funds
    require(
        sent_funds.amount == state.price.amount,
        ContractError::InsufficientFunds {},
    )?;
    let contract = CW721_CONTRACT.load(deps.storage)?;

    let timestamp_now = env.block.time.seconds();

    // Get the current block time from genesis time
    let from_genesis = timestamp_now - GENESIS_TIME;

    // Get the current round
    let _current_round = from_genesis / PERIOD;
    // const TERRAND_ADDRESS_MAINNET: &str = "terra1s90fm6hmh5n9drvucvv076ldemlqhe032qtjdq";

    let random_response: LatestRandomResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: TERRAND_ADDRESS_TESTNET.to_string(),
            msg: encode_binary(&terrand::msg::QueryMsg::LatestDrand {})?,
        }))?;
    let randomness = Binary::to_base64(&random_response.randomness);
    let vec = randomness.into_bytes();
    let ran_vec: Vec<u64> = vec.iter().map(|x| *x as u64).collect();
    // Concatinating the elements of the random number would yield an unworkably large number
    // So I opted for the sum, which is still random and large enough to work with modulus of list's length
    let mut random_number: u64 = ran_vec.iter().sum();
    // In case the random number is smaller than the number of NFTs
    while random_number < n_of_nfts as u64 {
        random_number *= 2;
    }
    // Use modulus to get a random index of the NFTs list
    let index = random_number as usize % n_of_nfts;
    // Select NFT & remove it from list at the same time. Used swap_remove since it's more efficient and the ordering doesn't matter
    let random_nft = list.swap_remove(index);
    LIST.save(deps.storage, &list)?;

    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract.clone().identifier,
            msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
                recipient: info.sender.to_string(),
                token_id: random_nft.clone(),
            })?,
            funds: vec![],
        }))
        .add_attribute("action", "claim")
        .add_attribute("token_id", random_nft)
        .add_attribute("token_contract", contract.identifier)
        .add_attribute("recipient", info.sender.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::AndrQuery(msg) => ADOContract::default().query(deps, env, msg, query),
        QueryMsg::NumberOfNFTs {} => encode_binary(&query_number_of_nfts(deps)?),
        QueryMsg::SaleDetails {} => encode_binary(&query_state(deps)?),
        QueryMsg::Status {} => encode_binary(&query_status(deps)?),
    }
}
fn query_status(deps: Deps) -> Result<StatusResponse, ContractError> {
    let status = STATUS.load(deps.storage)?;
    Ok(StatusResponse { status })
}

fn query_number_of_nfts(deps: Deps) -> Result<NumberOfNFTsResponse, ContractError> {
    let list = LIST.load(deps.storage)?;
    let number = list.len();
    Ok(NumberOfNFTsResponse { number })
}
fn query_state(deps: Deps) -> Result<StateResponse, ContractError> {
    let state = STATE.load(deps.storage)?;
    Ok(StateResponse { state })
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::ado_base::recipient::Recipient;
    use common::mission::AndrAddress;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coin, from_binary, to_binary};

    // fn mint(deps: DepsMut, token_id: impl Into<String>) -> Result<Response, ContractError> {
    //     let msg = ExecuteMsg::Mint(Box::new(MintMsg {
    //         token_id: token_id.into(),
    //         owner: mock_env().contract.address.to_string(),
    //         token_uri: None,
    //         extension: TokenExtension {
    //             name: "name".to_string(),
    //             publisher: "publisher".to_string(),
    //             description: None,
    //             transfer_agreement: None,
    //             metadata: None,
    //             archived: false,
    //             pricing: None,
    //         },
    //     }));
    //     println!("check 4");

    //     execute(deps, mock_env(), mock_info("owner", &[]), msg)
    // }
    #[test]
    fn swap_rem() {
        let mut v: Vec<u8> = vec![
            12, 4, 1, 84, 179, 120, 223, 73, 87, 30, 84, 218, 109, 137, 198, 191, 55, 238, 215,
            158, 129, 164, 35, 40, 96, 219, 56, 72, 174, 106, 132, 143,
        ];

        let removed_element = v.swap_remove(1);
        assert_eq!(removed_element, 4);
    }
    #[test]
    fn binary() {
        let v: Vec<u8> = vec![
            12, 4, 1, 84, 179, 120, 223, 73, 87, 30, 84, 218, 109, 137, 198, 191, 55, 238, 215,
            158, 129, 164, 35, 40, 96, 219, 56, 72, 174, 106, 132, 143,
        ];

        let bin = to_binary(&v).unwrap();
        println!("{:?}", bin);

        let random_hex: Vec<u8> = from_binary(&bin).unwrap();
        println!("{:?}", random_hex);
    }
    #[test]
    fn bytes() {
        let _v: Vec<u8> = vec![
            12, 4, 1, 84, 179, 120, 223, 73, 87, 30, 84, 218, 109, 137, 198, 191, 55, 238, 215,
            158, 129, 164, 35, 40, 96, 219, 56, 72, 174, 106, 132, 143,
        ];
        let n = "2b51af9c2bc12b262e2fc955bcb9fab4c89375efee6210385c40f59948e539d6".to_string();
        let tbin = Binary::from_base64(&n).unwrap();
        println!("from base64: {:?}", tbin);
        let bin = to_binary(&n).unwrap();
        println!(" to binary: {:?}", bin);
        let trandom = Binary::to_base64(&tbin);
        println!(" to_base64{:?}", trandom);
        let random_hex: String = from_binary(&bin).unwrap();
        println!(" from_binary{:?}", random_hex);

        let vec = trandom.into_bytes();
        println!("trandom into bytes{:?}", vec);
        let ran_vec: Vec<u64> = vec.iter().map(|x| *x as u64).collect();
        let random_number: u64 = ran_vec.iter().sum();
        println!("{:?}", random_number);
        let index = random_number % 3;
        println!("{:?}", index);
    }

    #[test]
    fn test_instantiate() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info("owner", &[]);
        let msg = InstantiateMsg {
            andromeda_cw721_contract: AndrAddress {
                identifier: "cw721_contract".to_string(),
            },
        };
        let res = instantiate(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(0, res.messages.len());
        let status = STATUS.load(&deps.storage).unwrap();
        assert!(!status);
    }
    #[test]
    fn test_sale_details_unauthorized() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info("owner", &[]);
        let msg = InstantiateMsg {
            andromeda_cw721_contract: AndrAddress {
                identifier: "cw721_contract".to_string(),
            },
        };
        instantiate(deps.as_mut(), env, info, msg).unwrap();
        let info = mock_info("anyone", &[]);
        let msg = ExecuteMsg::SaleDetails {
            price: coin(5, "uusd"),
            max_amount_per_wallet: Some(Uint128::from(1_u64)),
            recipient: Recipient::Addr("me".to_string()),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});
    }
    #[test]
    fn test_sale_details_invalid_price() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info("owner", &[]);
        let msg = InstantiateMsg {
            andromeda_cw721_contract: AndrAddress {
                identifier: "cw721_contract".to_string(),
            },
        };
        instantiate(deps.as_mut(), env, info, msg).unwrap();
        let info = mock_info("owner", &[]);
        let msg = ExecuteMsg::SaleDetails {
            price: coin(0, "uusd"),
            max_amount_per_wallet: Some(Uint128::from(1_u64)),
            recipient: Recipient::Addr("me".to_string()),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(res, ContractError::InvalidZeroAmount {});
    }
    #[test]
    fn test_sale_details_invalid_denomination() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info("owner", &[]);
        let msg = InstantiateMsg {
            andromeda_cw721_contract: AndrAddress {
                identifier: "cw721_contract".to_string(),
            },
        };
        instantiate(deps.as_mut(), env, info, msg).unwrap();
        let info = mock_info("owner", &[]);
        let msg = ExecuteMsg::SaleDetails {
            price: coin(10, "LUNA"),
            max_amount_per_wallet: Some(Uint128::from(1_u64)),
            recipient: Recipient::Addr("me".to_string()),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(
            res,
            ContractError::InvalidFunds {
                msg: "Only uusd is allowed".to_string(),
            }
        );
    }
    #[test]
    fn test_sale_details_max_amount_per_wallet() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info("owner", &[]);
        let msg = InstantiateMsg {
            andromeda_cw721_contract: AndrAddress {
                identifier: "cw721_contract".to_string(),
            },
        };
        instantiate(deps.as_mut(), env, info, msg).unwrap();
        let info = mock_info("owner", &[]);
        let msg = ExecuteMsg::SaleDetails {
            price: coin(10, "uusd"),
            max_amount_per_wallet: Some(Uint128::from(0_u64)),
            recipient: Recipient::Addr("me".to_string()),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(res, ContractError::InvalidZeroAmount {});
    }
    #[test]
    fn test_sale_details() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info("owner", &[]);
        let msg = InstantiateMsg {
            andromeda_cw721_contract: AndrAddress {
                identifier: "cw721_contract".to_string(),
            },
        };
        instantiate(deps.as_mut(), env, info, msg).unwrap();
        let info = mock_info("owner", &[]);
        let msg = ExecuteMsg::SaleDetails {
            price: coin(10, "uusd"),
            max_amount_per_wallet: Some(Uint128::from(1_u64)),
            recipient: Recipient::Addr("me".to_string()),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(
            res,
            Response::new().add_attributes(vec![
                attr("action", "switch status"),
                attr("price", coin(10, "uusd").to_string()),
                attr("max_amount_per_wallet", Uint128::from(1_u64)),
                attr("recipient", "me".to_string(),),
            ])
        );
    }
    #[test]
    fn test_switch_status() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info("owner", &[]);
        let msg = InstantiateMsg {
            andromeda_cw721_contract: AndrAddress {
                identifier: "cw721_contract".to_string(),
            },
        };
        instantiate(deps.as_mut(), env, info, msg).unwrap();
        let status = STATUS.load(&deps.storage).unwrap();
        assert!(!status);
        let info = mock_info("owner", &[]);
        execute_switch_status(deps.as_mut(), info).unwrap();
        let status = STATUS.load(&deps.storage).unwrap();
        assert!(status);
        let info = mock_info("owner", &[]);
        execute_switch_status(deps.as_mut(), info).unwrap();
        let status = STATUS.load(&deps.storage).unwrap();
        assert!(!status);
        let info = mock_info("anyone", &[]);
        let err = execute_switch_status(deps.as_mut(), info).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});
    }
    // #[test]
    // fn test_mint_successful() {
    //     let mut deps = mock_dependencies_custom(&[]);
    //     let env = mock_env();
    //     let info = mock_info("owner", &[]);
    //     let msg = InstantiateMsg {
    //         andromeda_cw721_contract: AndrAddress {
    //             identifier: "cw721_contract".to_string(),
    //         },
    //     };
    //     instantiate(deps.as_mut(), env, info, msg).unwrap();

    //     let res = mint(deps.as_mut(), "token_id").unwrap();

    //     let mint_msg = Box::new(MintMsg {
    //         token_id: "token_id".to_string(),
    //         owner: mock_env().contract.address.to_string(),
    //         token_uri: None,
    //         extension: TokenExtension {
    //             name: "name".to_string(),
    //             publisher: "publisher".to_string(),
    //             description: None,
    //             transfer_agreement: None,
    //             metadata: None,
    //             archived: false,
    //             pricing: None,
    //         },
    //     });

    //     assert_eq!(
    //         Response::new()
    //             .add_attribute("action", "mint")
    //             .add_message(WasmMsg::Execute {
    //                 contract_addr: MOCK_TOKEN_CONTRACT.to_owned(),
    //                 msg: encode_binary(&Cw721ExecuteMsg::Mint(mint_msg)).unwrap(),
    //                 funds: vec![],
    //             }),
    //         res
    //     );
    //     let list = LIST.load(&deps.storage).unwrap();

    //     assert_eq!(list.contains(&"token_id".to_string()), true);
    // }

    #[test]
    fn test_mint_unauthorized() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info("owner", &[]);
        let msg = InstantiateMsg {
            andromeda_cw721_contract: AndrAddress {
                identifier: "cw721_contract".to_string(),
            },
        };
        instantiate(deps.as_mut(), env, info, msg).unwrap();

        let msg = ExecuteMsg::Mint(Box::new(MintMsg {
            token_id: "token_id".to_string(),
            owner: mock_env().contract.address.to_string(),
            token_uri: None,
            extension: TokenExtension {
                name: "name".to_string(),
                publisher: "publisher".to_string(),
                description: None,
                transfer_agreement: None,
                metadata: None,
                archived: false,
                pricing: None,
            },
        }));
        let info = mock_info("not_owner", &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();

        assert_eq!(ContractError::Unauthorized {}, res);
    }
    #[test]
    fn test_mint_wrong_status() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info("owner", &[]);
        let msg = InstantiateMsg {
            andromeda_cw721_contract: AndrAddress {
                identifier: "cw721_contract".to_string(),
            },
        };
        instantiate(deps.as_mut(), env, info, msg).unwrap();
        let info = mock_info("owner", &[]);
        execute_switch_status(deps.as_mut(), info).unwrap();
        let msg = ExecuteMsg::Mint(Box::new(MintMsg {
            token_id: "token_id".to_string(),
            owner: mock_env().contract.address.to_string(),
            token_uri: None,
            extension: TokenExtension {
                name: "name".to_string(),
                publisher: "publisher".to_string(),
                description: None,
                transfer_agreement: None,
                metadata: None,
                archived: false,
                pricing: None,
            },
        }));
        let info = mock_info("owner", &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();

        assert_eq!(ContractError::NotInRefillMode {}, res);
    }

    // #[test]
    // fn test_buy_refill() {
    //     let mut deps = mock_dependencies(&[]);
    //     let env = mock_env();
    //     let info = mock_info("owner", &[]);
    //     let msg = InstantiateMsg {
    //         andromeda_cw721_contract: AndrAddress {
    //             identifier: "cw721_contract".to_string(),
    //         },
    //     };
    //     instantiate(deps.as_mut(), env, info, msg).unwrap();
    //     let info = mock_info("owner", &[]);
    //     let msg = ExecuteMsg::SaleDetails {
    //         price: coin(10, "uusd"),
    //         max_amount_per_wallet: Some(Uint128::from(1_u64)),
    //         recipient: Recipient::Addr("me".to_string()),
    //     };
    //     execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    //     let info = mock_info("owner", &[]);

    //     let mint_msg = ExecuteMsg::Mint(Box::new(MintMsg {
    //         token_id: "token_id".to_string(),
    //         owner: "not_crowdfund".to_string(),
    //         token_uri: None,
    //         extension: TokenExtension {
    //             name: "name".to_string(),
    //             publisher: "publisher".to_string(),
    //             description: None,
    //             transfer_agreement: None,
    //             metadata: None,
    //             archived: false,
    //             pricing: None,
    //         },
    //     }));
    //     execute(deps.as_mut(), mock_env(), info, mint_msg).unwrap();

    //     let info = mock_info("anyone", &[coin(10, "uusd")]);
    //     let msg = ExecuteMsg::Buy {};
    //     let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    //     assert_eq!(err, ContractError::Refilling {});
    // }
    // #[test]
    // fn test_buy_insufficient_funds() {
    //     let mut deps = mock_dependencies(&[]);
    //     let env = mock_env();
    //     let info = mock_info("owner", &[]);
    //     let msg = InstantiateMsg {
    //         andromeda_cw721_contract: AndrAddress {
    //             identifier: "cw721_contract".to_string(),
    //         },
    //     };
    //     instantiate(deps.as_mut(), env, info, msg).unwrap();
    //     let info = mock_info("owner", &[]);
    //     let msg = ExecuteMsg::SaleDetails {
    //         price: coin(10, "uusd"),
    //         max_amount_per_wallet: Some(Uint128::from(1_u64)),
    //         recipient: Recipient::Addr("me".to_string()),
    //     };
    //     execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    //     let info = mock_info("owner", &[]);

    //     let mint_msg = ExecuteMsg::Mint(Box::new(MintMsg {
    //         token_id: "token_id".to_string(),
    //         owner: "not_crowdfund".to_string(),
    //         token_uri: None,
    //         extension: TokenExtension {
    //             name: "name".to_string(),
    //             publisher: "publisher".to_string(),
    //             description: None,
    //             transfer_agreement: None,
    //             metadata: None,
    //             archived: false,
    //             pricing: None,
    //         },
    //     }));
    //     execute(deps.as_mut(), mock_env(), info, mint_msg).unwrap();
    //     // Sets status to true, allowing purchasing
    //     let info = mock_info("owner", &[]);
    //     execute_switch_status(deps.as_mut(), info).unwrap();

    //     let info = mock_info("anyone", &[coin(9, "uusd")]);
    //     let msg = ExecuteMsg::Buy {};
    //     let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    //     assert_eq!(err, ContractError::InsufficientFunds {});
    // }
    // #[test]
    // fn test_buy_wrong_denom() {
    //     let mut deps = mock_dependencies(&[]);
    //     let env = mock_env();
    //     let info = mock_info("owner", &[]);
    //     let msg = InstantiateMsg {
    //         andromeda_cw721_contract: AndrAddress {
    //             identifier: "cw721_contract".to_string(),
    //         },
    //     };
    //     instantiate(deps.as_mut(), env, info, msg).unwrap();
    //     let info = mock_info("owner", &[]);
    //     let msg = ExecuteMsg::SaleDetails {
    //         price: coin(10, "uusd"),
    //         max_amount_per_wallet: Some(Uint128::from(1_u64)),
    //         recipient: Recipient::Addr("me".to_string()),
    //     };
    //     execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    //     let info = mock_info("owner", &[]);

    //     let mint_msg = ExecuteMsg::Mint(Box::new(MintMsg {
    //         token_id: "token_id".to_string(),
    //         owner: "not_crowdfund".to_string(),
    //         token_uri: None,
    //         extension: TokenExtension {
    //             name: "name".to_string(),
    //             publisher: "publisher".to_string(),
    //             description: None,
    //             transfer_agreement: None,
    //             metadata: None,
    //             archived: false,
    //             pricing: None,
    //         },
    //     }));
    //     execute(deps.as_mut(), mock_env(), info, mint_msg).unwrap();
    //     // Sets status to true, allowing purchasing
    //     let info = mock_info("owner", &[]);
    //     execute_switch_status(deps.as_mut(), info).unwrap();

    //     let info = mock_info("anyone", &[coin(10, "euro")]);
    //     let msg = ExecuteMsg::Buy {};
    //     let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    //     assert_eq!(
    //         err,
    //         ContractError::InvalidFunds {
    //             msg: "Only uusd is accepted".to_string(),
    //         }
    //     );
    // }
    #[test]
    fn test_buy_no_nfts() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info("owner", &[]);
        let msg = InstantiateMsg {
            andromeda_cw721_contract: AndrAddress {
                identifier: "cw721_contract".to_string(),
            },
        };
        instantiate(deps.as_mut(), env, info, msg).unwrap();
        let info = mock_info("owner", &[]);
        let msg = ExecuteMsg::SaleDetails {
            price: coin(10, "uusd"),
            max_amount_per_wallet: Some(Uint128::from(1_u64)),
            recipient: Recipient::Addr("me".to_string()),
        };
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Sets status to true, allowing purchasing
        let info = mock_info("owner", &[]);
        execute_switch_status(deps.as_mut(), info).unwrap();

        let info = mock_info("anyone", &[coin(10, "euro")]);
        let msg = ExecuteMsg::Buy {};
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(err, ContractError::OutOfNFTs {});
    }
    // #[test]
    // fn test_buy_successful() {
    //     let mut deps = mock_dependencies(&[]);
    //     let env = mock_env();
    //     let info = mock_info("owner", &[]);
    //     let msg = InstantiateMsg {
    //         andromeda_cw721_contract: AndrAddress {
    //             identifier: "cw721_contract".to_string(),
    //         },
    //     };
    //     instantiate(deps.as_mut(), env, info, msg).unwrap();
    //     let info = mock_info("owner", &[]);
    //     let msg = ExecuteMsg::SaleDetails {
    //         price: coin(10, "uusd"),
    //         max_amount_per_wallet: Some(Uint128::from(1_u64)),
    //         recipient: Recipient::Addr("me".to_string()),
    //     };
    //     execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    //     let info = mock_info("owner", &[]);

    //     let mint_msg = ExecuteMsg::Mint(Box::new(MintMsg {
    //         token_id: "token_id".to_string(),
    //         owner: "not_crowdfund".to_string(),
    //         token_uri: None,
    //         extension: TokenExtension {
    //             name: "name".to_string(),
    //             publisher: "publisher".to_string(),
    //             description: None,
    //             transfer_agreement: None,
    //             metadata: None,
    //             archived: false,
    //             pricing: None,
    //         },
    //     }));
    //     execute(deps.as_mut(), mock_env(), info, mint_msg).unwrap();
    //     // Sets status to true, allowing purchasing
    //     let info = mock_info("owner", &[]);
    //     execute_switch_status(deps.as_mut(), info).unwrap();

    //     let info = mock_info("anyone", &[coin(10, "uusd")]);
    //     let msg = ExecuteMsg::Buy {};
    //     let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    //     assert_eq!(
    //         res,
    //         Response::new()
    //             .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
    //                 contract_addr: MOCK_TOKEN_CONTRACT.to_string(),
    //                 msg: encode_binary(&Cw721ExecuteMsg::TransferNft {
    //                     recipient: info.sender.to_string(),
    //                     token_id: "token_id".to_string(),
    //                 })
    //                 .unwrap(),
    //                 funds: vec![],
    //             }))
    //             .add_attribute("action", "claim")
    //             .add_attribute("token_id", "token_id")
    //             .add_attribute("token_contract", MOCK_TOKEN_CONTRACT.to_string())
    //             .add_attribute("recipient", info.sender.to_string().clone())
    //     );
    // }
}
