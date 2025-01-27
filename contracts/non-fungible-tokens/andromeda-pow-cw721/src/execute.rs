use andromeda_non_fungible_tokens::{
    cw721::{ExecuteMsg as AndrCw721ExecuteMsg, TokenExtension},
    pow_cw721::{ExecuteMsg, PowNFTInfo},
};
use andromeda_std::{
    ado_contract::ADOContract,
    amp::AndrAddr,
    common::{actions::call_action, context::ExecuteContext, encode_binary},
    error::ContractError,
};
use cosmwasm_std::{Binary, CosmosMsg, Event, Response, WasmMsg};
use sha2::{Digest, Sha256};

use crate::contract::MINT_POW_NFT_ACTION;
use crate::state::{LINKED_CW721_ADDRESS, POW_NFT};

pub fn handle_execute(mut ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    let action_response = call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;

    let res = match msg {
        ExecuteMsg::MintPowNFT {
            owner,
            token_id,
            token_uri,
            extension,
            base_difficulty,
        } => execute_mint_pow_nft(ctx, owner, token_id, token_uri, extension, base_difficulty),
        ExecuteMsg::SubmitProof { token_id, solution } => {
            execute_submit_proof(ctx, token_id, solution)
        }
        _ => ADOContract::default().execute(ctx, msg),
    }?;

    Ok(res
        .add_submessages(action_response.messages)
        .add_attributes(action_response.attributes)
        .add_events(action_response.events))
}

fn execute_mint_pow_nft(
    mut ctx: ExecuteContext,
    owner: AndrAddr,
    token_id: String,
    token_uri: Option<String>,
    extension: TokenExtension,
    base_difficulty: u64,
) -> Result<Response, ContractError> {
    if base_difficulty == 0 || base_difficulty > 128 {
        return Err(ContractError::CustomError {
            msg: "Base difficulty must be between 1 and 128".to_string(),
        });
    }

    let sender = ctx.info.sender;

    ADOContract::default().is_permissioned(
        ctx.deps.branch(),
        ctx.env.clone(),
        MINT_POW_NFT_ACTION,
        sender.clone(),
    )?;

    if POW_NFT
        .may_load(ctx.deps.storage, token_id.clone())?
        .is_some()
    {
        return Err(ContractError::CustomError {
            msg: format!("Token ID {} already exists", token_id),
        });
    }

    let owner_addr = owner.get_raw_address(&ctx.deps.as_ref())?;
    let cw721_address = LINKED_CW721_ADDRESS.load(ctx.deps.storage)?;

    let addr = cw721_address
        .get_raw_address(&ctx.deps.as_ref())?
        .to_string();
    let mint_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: addr,
        msg: encode_binary(&AndrCw721ExecuteMsg::Mint {
            token_id: token_id.clone(),
            owner: owner_addr.to_string(),
            token_uri,
            extension,
        })?,
        funds: vec![],
    });

    let block_height = ctx.env.block.height;

    let mut hasher = Sha256::new();
    hasher.update(block_height.to_be_bytes());
    let last_hash = hasher.finalize().to_vec();

    let pow_nft_info = PowNFTInfo {
        owner: owner_addr,
        level: 1_u64,
        last_hash: Binary(last_hash),
        difficulty: base_difficulty,
    };

    POW_NFT.save(ctx.deps.storage, token_id, &pow_nft_info)?;

    Ok(Response::new()
        .add_message(mint_msg)
        .add_attribute("method", "mint_pow_nft")
        .add_attribute("sender", sender))
}

fn execute_submit_proof(
    ctx: ExecuteContext,
    token_id: String,
    solution: u128,
) -> Result<Response, ContractError> {
    let sender = ctx.info.sender;
    let mut pow_nft = POW_NFT
        .load(ctx.deps.storage, token_id.clone())
        .map_err(|_| ContractError::NFTNotFound {})?;

    let mut hasher = Sha256::new();
    hasher.update(&pow_nft.last_hash);
    hasher.update(&solution.to_be_bytes());
    let hash = hasher.finalize();

    let hash_value = u128::from_be_bytes(hash[0..16].try_into().unwrap());
    let threshold = u128::MAX >> (pow_nft.difficulty as u32);

    if hash_value > threshold {
        return Err(ContractError::CustomError {
            msg: "Proof does not meet difficulty".to_string(),
        });
    }

    pow_nft.difficulty = if pow_nft.difficulty >= 2 {
        let next_difficulty = (pow_nft.difficulty as f64 * 1.5) as u64;
        if next_difficulty > 128 {
            return Err(ContractError::CustomError {
                msg: format!(
                    "Max difficulty is 128. Next difficulty will be over 128. Current level: {:?}",
                    pow_nft.level
                ),
            });
        }
        next_difficulty
    } else {
        2
    };

    pow_nft.level += 1;

    let block_height = ctx.env.block.height;
    let nonce = ctx
        .env
        .transaction
        .ok_or_else(|| ContractError::CustomError {
            msg: "Transaction info not available".to_string(),
        })?;

    let mut hasher = Sha256::new();
    hasher.update(&pow_nft.last_hash);
    hasher.update(&solution.to_be_bytes());
    hasher.update(&block_height.to_be_bytes());
    hasher.update(&nonce.index.to_be_bytes());
    let hash = hasher.finalize();
    pow_nft.last_hash = Binary(hash.to_vec());

    POW_NFT.save(ctx.deps.storage, token_id.clone(), &pow_nft)?;

    Ok(Response::new()
        .add_attribute("method", "submit_proof")
        .add_attribute("sender", sender)
        .add_attribute("token_id", token_id.clone())
        .add_attribute("new_level", pow_nft.level.to_string())
        .add_attribute("new_difficulty", pow_nft.difficulty.to_string())
        .add_event(
            Event::new("pow_nft_level_up")
                .add_attribute("token_id", token_id)
                .add_attribute("new_level", pow_nft.level.to_string())
                .add_attribute("new_difficulty", pow_nft.difficulty.to_string()),
        ))
}
