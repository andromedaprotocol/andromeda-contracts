#![cfg(all(not(target_arch = "wasm32"), feature = "testing"))]

use crate::contract::{execute, instantiate, query};
use andromeda_non_fungible_tokens::cw721::{
    ExecuteMsg, InstantiateMsg, MintMsg, QueryMsg, TokenExtension, TransferAgreement,
};
use andromeda_std::{ado_base::modules::Module, amp::addresses::AndrAddr};
use cosmwasm_std::{Binary, Coin, Empty};
use cw_multi_test::{Contract, ContractWrapper};

pub fn mock_andromeda_cw721() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new_with_empty(execute, instantiate, query);
    Box::new(contract)
}

pub fn mock_cw721_instantiate_msg(
    name: String,
    symbol: String,
    minter: impl Into<String>,
    modules: Option<Vec<Module>>,
    kernel_address: String,
    owner: Option<String>,
) -> InstantiateMsg {
    InstantiateMsg {
        name,
        symbol,
        minter: AndrAddr::from_string(minter.into()),
        modules,
        kernel_address,
        owner,
    }
}

pub fn mock_cw721_owner_of(token_id: String, include_expired: Option<bool>) -> QueryMsg {
    QueryMsg::OwnerOf {
        token_id,
        include_expired,
    }
}

pub fn mock_cw721_minter_query() -> QueryMsg {
    QueryMsg::Minter {}
}

pub fn mock_mint_msg(
    token_id: String,
    extension: TokenExtension,
    token_uri: Option<String>,
    owner: String,
) -> MintMsg {
    MintMsg {
        token_id,
        owner,
        token_uri,
        extension,
    }
}

pub fn mock_quick_mint_msg(amount: u32, owner: String) -> ExecuteMsg {
    let mut mint_msgs: Vec<MintMsg> = Vec::new();
    for i in 0..amount {
        let extension = TokenExtension {
            publisher: owner.clone(),
        };

        let msg = mock_mint_msg(i.to_string(), extension, None, owner.clone());
        mint_msgs.push(msg);
    }

    ExecuteMsg::BatchMint { tokens: mint_msgs }
}

pub fn mock_send_nft(contract: String, token_id: String, msg: Binary) -> ExecuteMsg {
    ExecuteMsg::SendNft {
        contract,
        token_id,
        msg,
    }
}

pub fn mock_transfer_nft(recipient: String, token_id: String) -> ExecuteMsg {
    ExecuteMsg::TransferNft {
        recipient,
        token_id,
    }
}

pub fn mock_transfer_agreement(amount: Coin, purchaser: String) -> TransferAgreement {
    TransferAgreement { amount, purchaser }
}

pub fn mock_create_transfer_agreement_msg(
    token_id: String,
    agreement: Option<TransferAgreement>,
) -> ExecuteMsg {
    ExecuteMsg::TransferAgreement {
        token_id,
        agreement,
    }
}
