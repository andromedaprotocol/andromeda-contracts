use andromeda_math::shunting::ShuntingResponse;
#[cfg(not(feature = "library"))]
use andromeda_math::shunting::{
    EvaluateParam, EvaluateRefParam, ExecuteMsg, InstantiateMsg, QueryMsg,
};
use andromeda_std::{
    ado_base::{InstantiateMsg as BaseInstantiateMsg, MigrateMsg},
    ado_contract::ADOContract,
    common::{context::ExecuteContext, encode_binary},
    error::ContractError,
};
use cosmwasm_std::{
    attr, ensure, entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError,
    WasmQuery,
};
use cw2::set_contract_version;
use cw_utils::nonpayable;

use serde_cw_value::Value;

use crate::state::EXPRESSIONS;
use cw_json::JSON;
use simple_shunting::*;

const CONTRACT_NAME: &str = "crates.io:andromeda-shunting";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    EXPRESSIONS.save(deps.storage, &msg.expressions)?;

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
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let _contract = ADOContract::default();
    let ctx = ExecuteContext::new(deps, info, env);

    match msg {
        ExecuteMsg::AMPReceive(pkt) => {
            ADOContract::default().execute_amp_receive(ctx, pkt, handle_execute)
        }
        _ => handle_execute(ctx, msg),
    }
}

pub fn handle_execute(ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateExpressions { expressions } => {
            execute_update_expression(ctx, expressions)
        }
        _ => ADOContract::default().execute(ctx, msg),
    }
}

fn execute_update_expression(
    ctx: ExecuteContext,
    expressions: Vec<String>,
) -> Result<Response, ContractError> {
    let ExecuteContext { deps, info, .. } = ctx;
    nonpayable(&info)?;

    ensure!(
        ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    EXPRESSIONS.save(deps.storage, &expressions)?;

    Ok(Response::new().add_attributes(vec![attr("action", "update_expression")]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Evaluate { params } => encode_binary(&handle_eval_expression(deps, params)?),
        _ => ADOContract::default().query(deps, env, msg),
    }
}

fn handle_eval_expression(
    deps: Deps,
    params: Vec<EvaluateParam>,
) -> Result<ShuntingResponse, ContractError> {
    let expressions = EXPRESSIONS.load(deps.storage)?;
    let mut results: Vec<f64> = Vec::new();
    let mut result: f64 = 0.0;

    // replace parameters that require other shunting
    let params = parse_params(deps, params)?;

    for (index, expression) in expressions.iter().enumerate() {
        let mut parsed_expression = expression.to_string();

        // replace x0, x1 ... with actual params in expression
        for (ndx, param) in params.iter().enumerate() {
            let placeholder = format!("{{x{}}}", ndx);
            parsed_expression = parsed_expression.replace(&placeholder, param);
        }

        // replace y0, y1 ... with calculation results
        for (i, sub_result) in results.iter().enumerate().take(index) {
            let placeholder = format!("{{y{}}}", i);
            parsed_expression = parsed_expression.replace(&placeholder, &sub_result.to_string());
        }

        result = eval(&parsed_expression).unwrap();
        results.push(result);
    }

    Ok(ShuntingResponse {
        result: result.to_string(),
    })
}

fn parse_params(deps: Deps, params: Vec<EvaluateParam>) -> Result<Vec<String>, ContractError> {
    let mut parsed_params = Vec::new();

    for param in params {
        match param {
            EvaluateParam::Value(val) => parsed_params.push(val),
            EvaluateParam::Reference(val) => {
                let EvaluateRefParam {
                    contract,
                    msg,
                    accessor,
                } = val;
                let query_msg = WasmQuery::Smart {
                    contract_addr: contract.to_string(),
                    msg: Binary::from_base64(&msg)?,
                }
                .into();

                let json: JSON = deps.querier.query(&query_msg).unwrap();
                let Some(Value::String(val)) = json.get(&accessor).unwrap() else {
                    return Err(ContractError::InvalidExpression {
                        msg: format!("Invalid Accessor {}", accessor),
                    });
                };

                parsed_params.push(val.to_string());
            }
        }
    }
    Ok(parsed_params)
}

fn eval(expr: &str) -> Result<f64, ContractError> {
    let parsed_expr = ShuntingParser::parse_str(expr);
    if parsed_expr.is_err() {
        return Err(ContractError::InvalidExpression {
            msg: format!("Unable to parse expression: {}", expr),
        });
    };

    let result = MathContext::new().eval(&parsed_expr.unwrap());
    if let Err(_msg) = result {
        return Err(ContractError::InvalidExpression {
            msg: format!("Failed to evaulate the expressoin: {}", expr),
        });
    }

    Ok(result.unwrap())
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
