use andromeda_finance::cross_chain_swap::{
    ExecuteMsg, InstantiateMsg, OsmosisSwapResponse, QueryMsg,
};
use andromeda_std::common::actions::call_action;
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    amp::{
        messages::{AMPMsg, AMPPkt},
        AndrAddr,
    },
    error::ContractError,
};
use andromeda_std::{ado_contract::ADOContract, common::context::ExecuteContext};
use cosmwasm_std::{
    attr, entry_point, Binary, Coin, Decimal, Deps, DepsMut, Env, MessageInfo, Reply, Response,
    StdError, SubMsg,
};

use cw_utils::one_coin;

use crate::{
    dex::{execute_swap_osmo, parse_swap_reply, MSG_FORWARD_ID, MSG_SWAP_ID},
    state::{ForwardReplyState, FORWARD_REPLY_STATE},
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-cross-chain-swap";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let inst_resp = ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        &deps.querier,
        info,
        BaseInstantiateMsg {
            ado_type: CONTRACT_NAME.to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            kernel_address: msg.kernel_address,
            owner: msg.owner,
        },
    )?;

    Ok(inst_resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    //TODO: Handle recovery for failed swap
    deps.api.debug(format!("Reply: {msg:?}").as_str());
    match msg.id {
        MSG_SWAP_ID => {
            deps.api.debug("Handling Reply");
            // Load and clear forward state
            let state = FORWARD_REPLY_STATE.load(deps.storage)?;
            FORWARD_REPLY_STATE.remove(deps.storage);
            if msg.result.is_err() {
                Err(ContractError::Std(StdError::generic_err(
                    msg.result.unwrap_err(),
                )))
            } else {
                match state.dex.as_str() {
                    "osmo" => {
                        let swap_resp: OsmosisSwapResponse = parse_swap_reply(msg)?;
                        let funds = vec![Coin {
                            denom: swap_resp.token_out_denom.clone(),
                            amount: swap_resp.amount,
                        }];
                        let mut pkt = if let Some(amp_ctx) = state.amp_ctx {
                            AMPPkt::new(amp_ctx.get_origin(), amp_ctx.get_previous_sender(), vec![])
                        } else {
                            AMPPkt::new(env.contract.address.clone(), env.contract.address, vec![])
                        };
                        let msg = AMPMsg::new(
                            state.addr.clone(),
                            state.msg.clone().unwrap_or_default(),
                            Some(funds.clone()),
                        );
                        pkt = pkt.add_message(msg);
                        let kernel_address =
                            ADOContract::default().get_kernel_address(deps.as_ref().storage)?;
                        let sub_msg =
                            pkt.to_sub_msg(kernel_address.clone(), Some(funds), MSG_FORWARD_ID)?;
                        let mut resp = Response::default();
                        resp = resp.add_submessage(sub_msg).add_attributes(vec![
                            attr("action", "osmo_swap_and_forward_success"),
                            attr("to_denom", swap_resp.token_out_denom),
                            attr("to_amount", swap_resp.amount),
                            attr("forward_addr", state.addr),
                            attr("kernel_address", kernel_address),
                        ]);
                        Ok(resp)
                    }
                    _ => Err(ContractError::Std(StdError::generic_err("Unsupported dex"))),
                }
            }
        }
        MSG_FORWARD_ID => {
            if msg.result.is_err() {
                return Err(ContractError::Std(StdError::generic_err(
                    msg.result.unwrap_err(),
                )));
            }

            Ok(Response::default()
                .add_attributes(vec![attr("action", "message_forwarded_success")]))
        }
        _ => Err(ContractError::Std(StdError::GenericErr {
            msg: "Invalid Reply ID".to_string(),
        })),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let ctx = ExecuteContext::new(deps, info, env);

    match msg {
        ExecuteMsg::AMPReceive(pkt) => {
            ADOContract::default().execute_amp_receive(ctx, pkt, handle_execute)
        }
        _ => handle_execute(ctx, msg),
    }
}

pub fn handle_execute(mut ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    let _contract = ADOContract::default();
    call_action(
        &mut ctx.deps,
        &ctx.info,
        &ctx.env,
        &ctx.amp_ctx,
        msg.as_ref(),
    )?;
    match msg {
        ExecuteMsg::SwapAndForward {
            dex,
            to_denom,
            forward_addr,
            forward_msg,
            slippage_percentage,
            window_seconds,
        } => execute_swap_and_forward(
            ctx,
            dex,
            to_denom,
            forward_addr,
            forward_msg,
            slippage_percentage,
            window_seconds,
        ),
        _ => Err(ContractError::UnsupportedOperation {}),
    }
}

fn execute_swap_and_forward(
    ctx: ExecuteContext,
    dex: String,
    to_denom: String,
    forward_addr: AndrAddr,
    forward_msg: Option<Binary>,
    slippage_percentage: Decimal,
    window_seconds: Option<u64>,
) -> Result<Response, ContractError> {
    let msg: SubMsg;
    let input_coin = one_coin(&ctx.info)?;
    if FORWARD_REPLY_STATE
        .may_load(ctx.deps.as_ref().storage)?
        .is_some()
    {
        return Err(ContractError::Unauthorized {});
    }

    let amp_ctx = if let Some(pkt) = ctx.amp_ctx.clone() {
        Some(pkt.ctx)
    } else {
        None
    };

    FORWARD_REPLY_STATE.save(
        ctx.deps.storage,
        &ForwardReplyState {
            addr: forward_addr,
            msg: forward_msg,
            dex: dex.clone(),
            amp_ctx,
        },
    )?;

    match dex.as_str() {
        "osmo" => {
            msg = execute_swap_osmo(
                ctx,
                input_coin,
                to_denom,
                slippage_percentage,
                window_seconds,
            )?;
        }
        _ => {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: "Unsupported Dex".to_string(),
            }))
        }
    }

    Ok(Response::default().add_submessage(msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ADOContract::default().migrate(deps, env, CONTRACT_NAME, CONTRACT_VERSION)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    ADOContract::default().query(deps, env, msg)
}
