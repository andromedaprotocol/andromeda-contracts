#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, ensure, from_json, has_coins, to_json_binary, Addr, Api, BankMsg, Binary, Coin,
    CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo, QuerierWrapper, Reply, Response, StdError,
    SubMsg, Uint128,
};

use crate::state::{is_archived, ANDR_MINTER, ARCHIVED, TRANSFER_AGREEMENTS};
use andromeda_non_fungible_tokens::cw721::{
    ExecuteMsg, InstantiateMsg, MintMsg, QueryMsg, TokenExtension, TransferAgreement,
};
use andromeda_std::common::rates::get_tax_amount;
use andromeda_std::{
    ado_base::{AndromedaMsg, AndromedaQuery},
    ado_contract::{permissioning::is_context_permissioned_strict, ADOContract},
    amp::AndrAddr,
    common::{actions::call_action, context::ExecuteContext},
};

use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    common::Funds,
    error::ContractError,
};
use cw721::{ContractInfoResponse, Cw721Execute};
use cw721_base::{state::TokenInfo, Cw721Contract, ExecuteMsg as Cw721ExecuteMsg};

pub type AndrCW721Contract<'a> = Cw721Contract<'a, TokenExtension, Empty, ExecuteMsg, QueryMsg>;
const CONTRACT_NAME: &str = "crates.io:andromeda-cw721";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let contract_info = ContractInfoResponse {
        name: msg.name,
        symbol: msg.symbol,
    };

    // Do this directly instead of with cw721_contract.instantiate because we want to have minter
    // be an AndrAddress, which cannot be validated right away.
    AndrCW721Contract::default()
        .contract_info
        .save(deps.storage, &contract_info)?;

    let contract = ADOContract::default();
    ANDR_MINTER.save(deps.storage, &msg.minter)?;

    let resp = contract.instantiate(
        deps.storage,
        env,
        deps.api,
        &deps.querier,
        info.clone(),
        BaseInstantiateMsg {
            ado_type: CONTRACT_NAME.to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            kernel_address: msg.kernel_address,
            owner: msg.owner,
        },
    )?;

    Ok(resp.add_attributes(vec![attr("minter", msg.minter)]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    if let ExecuteMsg::AMPReceive(pkt) = msg {
        ADOContract::default().execute_amp_receive(
            ExecuteContext::new(deps, info, env),
            pkt,
            handle_execute,
        )
    } else {
        let ctx = ExecuteContext::new(deps, info, env);
        handle_execute(ctx, msg)
    }
}

fn handle_execute(mut ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    let action = msg.as_ref().to_string();

    let action_response = call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;

    if let ExecuteMsg::Approve { token_id, .. } = &msg {
        ensure!(
            !is_archived(ctx.deps.storage, token_id)?.is_archived,
            ContractError::TokenIsArchived {}
        );
    }

    let res = match msg {
        ExecuteMsg::Mint {
            token_id,
            token_uri,
            owner,
            extension,
        } => execute_mint(ctx, token_id, token_uri, owner, extension, action),
        ExecuteMsg::BatchMint { tokens } => execute_batch_mint(ctx, tokens, action),
        ExecuteMsg::TransferNft {
            recipient,
            token_id,
        } => execute_transfer(ctx, recipient, token_id, &action),
        ExecuteMsg::TransferAgreement {
            token_id,
            agreement,
        } => execute_update_transfer_agreement(ctx, token_id, agreement),
        ExecuteMsg::Archive { token_id } => execute_archive(ctx, token_id),
        ExecuteMsg::Burn { token_id } => execute_burn(ctx, token_id),
        ExecuteMsg::SendNft {
            contract,
            token_id,
            msg,
        } => execute_send_nft(ctx, token_id, contract, msg),
        _ => {
            let serialized = to_json_binary(&msg)?;

            match from_json::<AndromedaMsg>(&serialized) {
                Ok(msg) => contract.execute(ctx, msg),
                Err(_) => execute_cw721(ctx, msg.into()),
            }
        }
    }?;
    Ok(res
        .add_submessages(action_response.messages)
        .add_attributes(action_response.attributes)
        .add_events(action_response.events))
}

fn execute_cw721(
    ctx: ExecuteContext,
    msg: Cw721ExecuteMsg<TokenExtension, ExecuteMsg>,
) -> Result<Response, ContractError> {
    let contract = AndrCW721Contract::default();
    Ok(contract.execute(ctx.deps, ctx.env, ctx.info, msg)?)
}

fn execute_mint(
    mut ctx: ExecuteContext,
    token_id: String,
    token_uri: Option<String>,
    owner: String,
    extension: TokenExtension,
    action: String,
) -> Result<Response, ContractError> {
    let minter = ANDR_MINTER
        .load(ctx.deps.storage)?
        .get_raw_address(&ctx.deps.as_ref())?;
    ensure!(
        ctx.contains_sender(minter.as_str())
            | is_context_permissioned_strict(
                ctx.deps.branch(),
                &ctx.info,
                &ctx.env,
                &ctx.amp_ctx,
                action
            )?,
        ContractError::Unauthorized {}
    );
    mint(ctx, token_id, token_uri, owner, extension)
}

fn mint(
    ctx: ExecuteContext,
    token_id: String,
    token_uri: Option<String>,
    owner: String,
    extension: TokenExtension,
) -> Result<Response, ContractError> {
    let cw721_contract = AndrCW721Contract::default();
    let token = TokenInfo {
        owner: ctx.deps.api.addr_validate(&owner)?,
        approvals: vec![],
        token_uri,
        extension,
    };

    cw721_contract
        .tokens
        .update(ctx.deps.storage, &token_id, |old| match old {
            Some(_) => Err(ContractError::Claimed {}),
            None => Ok(token),
        })?;

    cw721_contract.increment_tokens(ctx.deps.storage)?;

    Ok(Response::new()
        .add_attribute("action", "mint")
        .add_attribute("minter", ctx.info.sender)
        .add_attribute("owner", owner)
        .add_attribute("token_id", token_id))
}

fn execute_batch_mint(
    mut ctx: ExecuteContext,
    tokens_to_mint: Vec<MintMsg>,
    action: String,
) -> Result<Response, ContractError> {
    let mut resp = Response::default();
    let minter = ANDR_MINTER
        .load(ctx.deps.storage)?
        .get_raw_address(&ctx.deps.as_ref())?;
    ensure!(
        ctx.contains_sender(minter.as_str())
            | is_context_permissioned_strict(
                ctx.deps.branch(),
                &ctx.info,
                &ctx.env,
                &ctx.amp_ctx,
                action
            )?,
        ContractError::Unauthorized {}
    );
    ensure!(
        !tokens_to_mint.is_empty(),
        ContractError::Std(cosmwasm_std::StdError::GenericErr {
            msg: String::from("No tokens to mint")
        })
    );
    for msg in tokens_to_mint {
        let ctx = ExecuteContext {
            deps: ctx.deps.branch(),
            info: ctx.info.clone(),
            env: ctx.env.clone(),
            amp_ctx: ctx.amp_ctx.clone(),
        };
        let mint_resp = mint(ctx, msg.token_id, msg.token_uri, msg.owner, msg.extension)?;
        resp = resp
            .add_attributes(mint_resp.attributes)
            .add_submessages(mint_resp.messages);
    }

    Ok(resp)
}

fn execute_transfer(
    ctx: ExecuteContext,
    recipient: AndrAddr,
    token_id: String,
    action: &str,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    let base_contract = ADOContract::default();
    // Reduce all responses into one.
    let mut resp = Response::new();
    let recipient_address = recipient.get_raw_address(&deps.as_ref())?.into_string();
    let contract = AndrCW721Contract::default();
    let mut token = contract.tokens.load(deps.storage, &token_id)?;
    ensure!(
        !is_archived(deps.storage, &token_id)?.is_archived,
        ContractError::TokenIsArchived {}
    );

    let tax_amount = if let Some(agreement) =
        &TRANSFER_AGREEMENTS.may_load(deps.storage, &token_id)?
    {
        let agreement_amount = get_transfer_agreement_amount(deps.api, &deps.querier, agreement)?;
        let transfer_response = base_contract.query_deducted_funds(
            deps.as_ref(),
            action,
            Funds::Native(agreement_amount.clone()),
        )?;

        match transfer_response {
            Some(mut transfer_response) => {
                let remaining_amount = transfer_response.leftover_funds.try_get_coin()?;
                let tax_amount = get_tax_amount(
                    &transfer_response.msgs,
                    agreement_amount.amount,
                    remaining_amount.amount,
                );
                transfer_response
                    .msgs
                    .push(SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                        to_address: token.owner.to_string(),
                        amount: vec![remaining_amount],
                    })));
                resp = resp.add_submessages(transfer_response.msgs);
                tax_amount
            }
            None => {
                let remaining_amount = Funds::Native(agreement_amount).try_get_coin()?;
                let tax_amount = Uint128::zero();
                let msg = SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                    to_address: token.owner.to_string(),
                    amount: vec![remaining_amount],
                }));
                resp = resp.add_submessage(msg);
                tax_amount
            }
        }
    } else {
        Uint128::zero()
    };

    check_can_send(deps.as_ref(), env, info, &token_id, &token, tax_amount)?;
    token.owner = deps.api.addr_validate(&recipient_address)?;
    token.approvals.clear();
    TRANSFER_AGREEMENTS.remove(deps.storage, &token_id);
    contract.tokens.save(deps.storage, &token_id, &token)?;
    Ok(resp
        .add_attribute("action", "transfer")
        .add_attribute("recipient", recipient_address))
}

fn get_transfer_agreement_amount(
    _api: &dyn Api,
    _querier: &QuerierWrapper,
    agreement: &TransferAgreement,
) -> Result<Coin, ContractError> {
    let agreement_amount = agreement.amount.clone();
    Ok(agreement_amount)
}

fn check_can_send(
    deps: Deps,
    env: Env,
    info: MessageInfo,
    token_id: &str,
    token: &TokenInfo<TokenExtension>,
    tax_amount: Uint128,
) -> Result<(), ContractError> {
    // owner can send
    if token.owner == info.sender {
        return Ok(());
    }

    // token purchaser can send if correct funds are sent
    if let Some(agreement) = &TRANSFER_AGREEMENTS.may_load(deps.storage, token_id)? {
        let agreement_amount = get_transfer_agreement_amount(deps.api, &deps.querier, agreement)?;
        ensure!(
            has_coins(
                &info.funds,
                &Coin {
                    denom: agreement_amount.denom.to_owned(),
                    // Ensure that the taxes came from the sender.
                    amount: agreement_amount.amount + tax_amount,
                },
            ),
            ContractError::InsufficientFunds {}
        );
        if agreement.purchaser == info.sender || agreement.purchaser == "*" {
            return Ok(());
        }
    }

    // any non-expired token approval can send
    if token
        .approvals
        .iter()
        .any(|apr| apr.spender == info.sender && !apr.is_expired(&env.block))
    {
        return Ok(());
    }

    // operator can send
    let op = AndrCW721Contract::default()
        .operators
        .may_load(deps.storage, (&token.owner, &info.sender))?;
    match op {
        Some(ex) => {
            if ex.is_expired(&env.block) {
                Err(ContractError::Unauthorized {})
            } else {
                Ok(())
            }
        }
        None => Err(ContractError::Unauthorized {}),
    }
}

fn execute_update_transfer_agreement(
    ctx: ExecuteContext,
    token_id: String,
    agreement: Option<TransferAgreement>,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;
    let contract = AndrCW721Contract::default();
    let token = contract.tokens.load(deps.storage, &token_id)?;
    ensure!(token.owner == info.sender, ContractError::Unauthorized {});
    ensure!(
        !is_archived(deps.storage, &token_id)?.is_archived,
        ContractError::TokenIsArchived {}
    );
    if let Some(xfer_agreement) = &agreement {
        TRANSFER_AGREEMENTS.save(deps.storage, &token_id, xfer_agreement)?;
        if xfer_agreement.purchaser != "*" {
            deps.api.addr_validate(&xfer_agreement.purchaser)?;
        }
    } else {
        TRANSFER_AGREEMENTS.remove(deps.storage, &token_id);
    }

    contract
        .tokens
        .save(deps.storage, token_id.as_str(), &token)?;

    Ok(Response::default())
}

fn execute_archive(ctx: ExecuteContext, token_id: String) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;
    ensure!(
        !is_archived(deps.storage, &token_id)?.is_archived,
        ContractError::TokenIsArchived {}
    );
    let contract = AndrCW721Contract::default();
    let token = contract.tokens.load(deps.storage, &token_id)?;
    ensure!(token.owner == info.sender, ContractError::Unauthorized {});

    ARCHIVED.save(deps.storage, &token_id, &true)?;

    contract.tokens.save(deps.storage, &token_id, &token)?;

    Ok(Response::default())
}

fn execute_burn(ctx: ExecuteContext, token_id: String) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;
    let contract = AndrCW721Contract::default();
    let token = contract.tokens.load(deps.storage, &token_id)?;
    ensure!(token.owner == info.sender, ContractError::Unauthorized {});
    ensure!(
        !is_archived(deps.storage, &token_id)?.is_archived,
        ContractError::TokenIsArchived {}
    );

    contract.tokens.remove(deps.storage, &token_id)?;

    // Decrement token count.
    let count = contract.token_count.load(deps.storage)?;
    contract.token_count.save(deps.storage, &(count - 1))?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "burn"),
        attr("token_id", token_id),
        attr("sender", info.sender),
    ]))
}

fn execute_send_nft(
    ctx: ExecuteContext,
    token_id: String,
    contract_addr: AndrAddr,
    msg: Binary,
) -> Result<Response, ContractError> {
    let ExecuteContext {
        deps, info, env, ..
    } = ctx;
    let contract = AndrCW721Contract::default();
    TRANSFER_AGREEMENTS.remove(deps.storage, &token_id);
    let contract_addr = contract_addr.get_raw_address(&deps.as_ref())?.into_string();

    Ok(contract.send_nft(deps, env, info, contract_addr, token_id, msg)?)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::IsArchived { token_id } => {
            Ok(to_json_binary(&is_archived(deps.storage, &token_id)?)?)
        }
        QueryMsg::TransferAgreement { token_id } => {
            Ok(to_json_binary(&query_transfer_agreement(deps, token_id)?)?)
        }
        QueryMsg::Minter {} => Ok(to_json_binary(&query_minter(deps)?)?),
        _ => {
            let serialized = to_json_binary(&msg)?;
            match from_json::<AndromedaQuery>(&serialized) {
                Ok(msg) => ADOContract::default().query(deps, env, msg),
                _ => Ok(AndrCW721Contract::default().query(deps, env, msg.into())?),
            }
        } // _ => Ok(AndrCW721Contract::default().query(deps, env, msg.into())?),
    }
}

pub fn query_transfer_agreement(
    deps: Deps,
    token_id: String,
) -> Result<Option<TransferAgreement>, ContractError> {
    Ok(TRANSFER_AGREEMENTS.may_load(deps.storage, &token_id)?)
}

pub fn query_minter(deps: Deps) -> Result<Addr, ContractError> {
    let minter = ANDR_MINTER.load(deps.storage)?;
    minter.get_raw_address(&deps)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, env, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.result.is_err() {
        return Err(ContractError::Std(StdError::generic_err(
            msg.result.unwrap_err(),
        )));
    }

    Ok(Response::default())
}
