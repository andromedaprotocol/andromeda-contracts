use crate::state::{Config, CONFIG};
use andromeda_protocol::{
    communication::{encode_binary, parse_message, AndromedaMsg, AndromedaQuery},
    error::ContractError,
    operators::{execute_update_operators, query_is_operator, query_operators},
    ownership::{execute_update_owner, query_contract_owner},
    rates::{ExecuteMsg, InstantiateMsg, PaymentsResponse, QueryMsg, RateInfo},
};
use cosmwasm_std::{attr, entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Response};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        owner: info.sender.to_string(),
        rates: msg.rates,
    };
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attributes(vec![attr("action", "instantiate"), attr("type", "rates")]))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AndrReceive(msg) => execute_andr_receive(deps, info, msg),
        ExecuteMsg::UpdateRates { rates } => execute_update_rates(deps, info, rates),
    }
}

fn execute_andr_receive(
    deps: DepsMut,
    info: MessageInfo,
    msg: AndromedaMsg,
) -> Result<Response, ContractError> {
    match msg {
        AndromedaMsg::Receive(data) => {
            let rates: Vec<RateInfo> = parse_message(data)?;
            execute_update_rates(deps, info, rates)
        }
        AndromedaMsg::UpdateOwner { address } => execute_update_owner(deps, info, address),
        AndromedaMsg::UpdateOperators { operators } => {
            execute_update_operators(deps, info, operators)
        }
    }
}

fn execute_update_rates(
    deps: DepsMut,
    info: MessageInfo,
    rates: Vec<RateInfo>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if config.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }
    config.rates = rates;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attributes(vec![attr("action", "update_rates")]))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::AndrQuery(msg) => handle_andromeda_query(deps, msg),
        QueryMsg::Payments {} => encode_binary(&query_payments(deps)?),
    }
}

fn handle_andromeda_query(deps: Deps, msg: AndromedaQuery) -> Result<Binary, ContractError> {
    match msg {
        AndromedaQuery::Get(_) => encode_binary(&query_payments(deps)?),
        AndromedaQuery::Owner {} => encode_binary(&query_contract_owner(deps)?),
        AndromedaQuery::Operators {} => encode_binary(&query_operators(deps)?),
        AndromedaQuery::IsOperator { address } => {
            encode_binary(&query_is_operator(deps, &address)?)
        }
    }
}

fn query_payments(deps: Deps) -> Result<PaymentsResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let rates = config.rates;

    Ok(PaymentsResponse { payments: rates })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{execute, instantiate, query};
    use andromeda_protocol::{
        communication::{encode_binary, AndromedaMsg, AndromedaQuery},
        modules::{FlatRate, Rate},
        rates::{InstantiateMsg, PaymentsResponse, QueryMsg, RateInfo},
    };
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{Addr, Uint128};

    #[test]
    fn test_instantiate_query() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let owner = "owner";
        let info = mock_info(owner, &[]);
        let rates = vec![
            RateInfo {
                rate: Rate::Percent(10),
                is_additive: true,
                description: Some("desc1".to_string()),
                receivers: vec![Addr::unchecked("")],
            },
            RateInfo {
                rate: Rate::Flat(FlatRate {
                    amount: Uint128::from(10u128),
                    denom: "uusd".to_string(),
                }),
                is_additive: false,
                description: Some("desc2".to_string()),
                receivers: vec![Addr::unchecked("")],
            },
        ];
        let msg = InstantiateMsg {
            rates: rates.clone(),
        };
        let res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        assert_eq!(0, res.messages.len());

        let payments = query(deps.as_ref(), env, QueryMsg::Payments {}).unwrap();

        assert_eq!(
            payments,
            encode_binary(&PaymentsResponse { payments: rates }).unwrap()
        );

        //Why does this test error?
        //let payments = query(deps.as_ref(), mock_env(), QueryMsg::Payments {}).is_err();
        //assert_eq!(payments, true);
    }

    #[test]
    fn test_andr_receive() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let owner = "owner";
        let info = mock_info(owner, &[]);
        let rates = vec![
            RateInfo {
                rate: Rate::Percent(10),
                is_additive: true,
                description: Some("desc1".to_string()),
                receivers: vec![Addr::unchecked("")],
            },
            RateInfo {
                rate: Rate::Flat(FlatRate {
                    amount: Uint128::from(10u128),
                    denom: "uusd".to_string(),
                }),
                is_additive: false,
                description: Some("desc2".to_string()),
                receivers: vec![Addr::unchecked("")],
            },
        ];
        let msg = InstantiateMsg { rates: vec![] };
        let _res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        let msg =
            ExecuteMsg::AndrReceive(AndromedaMsg::Receive(Some(encode_binary(&rates).unwrap())));

        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(
            Response::new().add_attributes(vec![attr("action", "update_rates")]),
            res
        );

        let msg = QueryMsg::AndrQuery(AndromedaQuery::Get(None));

        let payments = query(deps.as_ref(), env, msg).unwrap();

        assert_eq!(
            payments,
            encode_binary(&PaymentsResponse { payments: rates }).unwrap()
        );
    }
}
