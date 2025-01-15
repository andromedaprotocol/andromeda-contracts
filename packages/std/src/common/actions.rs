use crate::{
    ado_contract::{permissioning::is_context_permissioned, ADOContract},
    amp::messages::AMPPkt,
    error::ContractError,
    os::aos_querier::AOSQuerier,
};
use cosmwasm_std::{ensure, DepsMut, Env, MessageInfo, Response};

pub fn call_action(
    deps: &mut DepsMut,
    info: &MessageInfo,
    env: &Env,
    amp_ctx: &Option<AMPPkt>,
    action: &str,
) -> Result<Response, ContractError> {
    ensure!(
        is_context_permissioned(deps, info, env, amp_ctx, action)?,
        ContractError::Unauthorized {}
    );

    let payee = if let Some(amp_ctx) = amp_ctx.clone() {
        deps.api.addr_validate(amp_ctx.ctx.get_origin().as_str())?
    } else {
        info.sender.clone()
    };

    let adodb_addr = ADOContract::default().get_adodb_address(deps.storage, &deps.querier)?;
    let code_id = deps
        .querier
        .query_wasm_contract_info(env.contract.address.clone())?
        .code_id;

    // Check ADO type and fees in one chain
    match AOSQuerier::ado_type_getter(&deps.querier, &adodb_addr, code_id)?
        .and_then(|ado_type| {
            AOSQuerier::action_fee_getter(&deps.querier, &adodb_addr, &ado_type, action).ok()
        })
        .map(|_| {
            ADOContract::default().pay_fee(deps.storage, &deps.querier, action.to_owned(), payee)
        }) {
        Some(fee_msg) => Ok(Response::default().add_submessage(fee_msg?)),
        None => Ok(Response::default()),
    }
}
