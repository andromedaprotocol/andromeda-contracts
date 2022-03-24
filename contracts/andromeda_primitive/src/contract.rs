#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError};
use cw2::{get_contract_version, set_contract_version};

use crate::state::{DATA, DEFAULT_KEY};
use ado_base::state::ADOContract;
use andromeda_protocol::primitive::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use common::{
    ado_base::{AndromedaQuery, InstantiateMsg as BaseInstantiateMsg},
    encode_binary,
    error::ContractError,
    parse_message,
    primitive::{GetValueResponse, Primitive},
    require,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda_primitive";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    ADOContract::default().instantiate(
        deps.storage,
        deps.api,
        info,
        BaseInstantiateMsg {
            ado_type: "primitive".to_string(),
            operators: Some(msg.operators),
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
    match msg {
        ExecuteMsg::AndrReceive(msg) => {
            ADOContract::default().execute(deps, env, info, msg, execute)
        }
        ExecuteMsg::SetValue { name, value } => execute_set_value(deps, info, name, value),
        ExecuteMsg::DeleteValue { name } => execute_delete_value(deps, info, name),
    }
}

pub fn execute_set_value(
    deps: DepsMut,
    info: MessageInfo,
    name: Option<String>,
    value: Primitive,
) -> Result<Response, ContractError> {
    let sender = info.sender.to_string();
    require(
        ADOContract::default().is_owner_or_operator(deps.storage, &sender)?,
        ContractError::Unauthorized {},
    )?;
    if value.is_invalid() {
        return Err(ContractError::InvalidPrimitive {});
    }
    let name: &str = get_name_or_default(&name);
    DATA.update::<_, StdError>(deps.storage, name, |old| match old {
        Some(_) => Ok(value.clone()),
        None => Ok(value.clone()),
    })?;

    Ok(Response::new()
        .add_attribute("method", "set_value")
        .add_attribute("sender", sender)
        .add_attribute("name", name)
        .add_attribute("value", format!("{:?}", value)))
}

pub fn execute_delete_value(
    deps: DepsMut,
    info: MessageInfo,
    name: Option<String>,
) -> Result<Response, ContractError> {
    let sender = info.sender.to_string();
    require(
        ADOContract::default().is_owner_or_operator(deps.storage, &sender)?,
        ContractError::Unauthorized {},
    )?;
    let name = get_name_or_default(&name);
    DATA.remove(deps.storage, name);
    Ok(Response::new()
        .add_attribute("method", "delete_value")
        .add_attribute("sender", sender)
        .add_attribute("name", name))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let version = get_contract_version(deps.storage)?;
    if version.contract != CONTRACT_NAME {
        return Err(ContractError::CannotMigrate {
            previous_contract: version.contract,
        });
    }
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::AndrQuery(msg) => handle_andromeda_query(deps, env, msg),
    }
}

fn handle_andromeda_query(
    deps: Deps,
    env: Env,
    msg: AndromedaQuery,
) -> Result<Binary, ContractError> {
    match msg {
        AndromedaQuery::Get(data) => match data {
            // Treat no binary as request to get value with default key.
            None => encode_binary(&query_value(deps, None)?),
            Some(_) => {
                let name: String = parse_message(&data)?;
                encode_binary(&query_value(deps, Some(name))?)
            }
        },
        _ => ADOContract::default().query(deps, env, msg, query),
    }
}

fn query_value(deps: Deps, name: Option<String>) -> Result<GetValueResponse, ContractError> {
    let name = get_name_or_default(&name);
    let value = DATA.load(deps.storage, name)?;
    Ok(GetValueResponse {
        name: name.to_string(),
        value,
    })
}

fn get_name_or_default(name: &Option<String>) -> &str {
    match name {
        None => DEFAULT_KEY,
        Some(s) => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::ado_base::AndromedaMsg;
    use cosmwasm_std::from_binary;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};

    fn query_value_helper(
        deps: Deps,
        name: Option<String>,
    ) -> Result<GetValueResponse, ContractError> {
        let binary_option = name.map(|name| encode_binary(&name).unwrap());
        let res = query(
            deps,
            mock_env(),
            QueryMsg::AndrQuery(AndromedaQuery::Get(binary_option)),
        );
        match res {
            Ok(res) => Ok(from_binary(&res).unwrap()),
            Err(err) => Err(err),
        }
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg { operators: vec![] };
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn set_and_update_value_with_name() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg { operators: vec![] };
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let msg = ExecuteMsg::SetValue {
            name: Some("test1".to_string()),
            value: Primitive::String("value1".to_string()),
        };
        let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
        assert_eq!(
            Response::new()
                .add_attribute("method", "set_value")
                .add_attribute("sender", "creator")
                .add_attribute("name", "test1")
                .add_attribute("value", "String(\"value1\")"),
            res
        );

        let query_res: GetValueResponse =
            query_value_helper(deps.as_ref(), Some("test1".to_string())).unwrap();

        assert_eq!(
            GetValueResponse {
                name: "test1".to_string(),
                value: Primitive::String("value1".to_string())
            },
            query_res
        );

        // Update the value to something else
        let msg = ExecuteMsg::SetValue {
            name: Some("test1".to_string()),
            value: Primitive::String("value2".to_string()),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let query_res: GetValueResponse =
            query_value_helper(deps.as_ref(), Some("test1".to_string())).unwrap();

        assert_eq!(
            GetValueResponse {
                name: "test1".to_string(),
                value: Primitive::String("value2".to_string())
            },
            query_res
        );
    }

    #[test]
    fn set_and_update_value_without_name() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg { operators: vec![] };
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let msg = ExecuteMsg::SetValue {
            name: None,
            value: Primitive::String("value1".to_string()),
        };
        let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
        assert_eq!(
            Response::new()
                .add_attribute("method", "set_value")
                .add_attribute("sender", "creator")
                .add_attribute("name", DEFAULT_KEY)
                .add_attribute("value", "String(\"value1\")"),
            res
        );

        let query_res: GetValueResponse = query_value_helper(deps.as_ref(), None).unwrap();

        assert_eq!(
            GetValueResponse {
                name: DEFAULT_KEY.to_string(),
                value: Primitive::String("value1".to_string())
            },
            query_res
        );

        // Update the value to something else
        let msg = ExecuteMsg::SetValue {
            name: None,
            value: Primitive::String("value2".to_string()),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let query_res: GetValueResponse = query_value_helper(deps.as_ref(), None).unwrap();

        assert_eq!(
            GetValueResponse {
                name: DEFAULT_KEY.to_string(),
                value: Primitive::String("value2".to_string())
            },
            query_res
        );
    }

    #[test]
    fn cannot_set_nested_vector_primitive() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg { operators: vec![] };
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let msg = ExecuteMsg::SetValue {
            name: None,
            value: Primitive::Vec(vec![Primitive::Vec(vec![])]),
        };
        let res: Result<Response, ContractError> = execute(deps.as_mut(), mock_env(), info, msg);
        assert_eq!(ContractError::InvalidPrimitive {}, res.unwrap_err());
    }

    #[test]
    fn delete_value_with_name() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg { operators: vec![] };
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let msg = ExecuteMsg::SetValue {
            name: Some("test1".to_string()),
            value: Primitive::String("value1".to_string()),
        };
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let query_res: GetValueResponse =
            query_value_helper(deps.as_ref(), Some("test1".to_string())).unwrap();

        assert_eq!(
            GetValueResponse {
                name: "test1".to_string(),
                value: Primitive::String("value1".to_string())
            },
            query_res
        );

        let msg = ExecuteMsg::DeleteValue {
            name: Some("test1".to_string()),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(
            res,
            Response::new()
                .add_attribute("method", "delete_value")
                .add_attribute("sender", "creator")
                .add_attribute("name", "test1")
        );
        let query_res = query_value_helper(deps.as_ref(), Some("test1".to_string()));
        assert!(query_res.is_err());
    }

    #[test]
    fn delete_value_without_name() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg { operators: vec![] };
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let msg = ExecuteMsg::SetValue {
            name: None,
            value: Primitive::String("value1".to_string()),
        };
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let query_res: GetValueResponse = query_value_helper(deps.as_ref(), None).unwrap();

        assert_eq!(
            GetValueResponse {
                name: DEFAULT_KEY.to_string(),
                value: Primitive::String("value1".to_string())
            },
            query_res
        );

        let msg = ExecuteMsg::DeleteValue { name: None };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(
            res,
            Response::new()
                .add_attribute("method", "delete_value")
                .add_attribute("sender", "creator")
                .add_attribute("name", DEFAULT_KEY)
        );
        let query_res = &query_value_helper(deps.as_ref(), None);
        assert!(query_res.is_err());
    }

    #[test]
    fn non_creator_cannot_set_value() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg { operators: vec![] };
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let user1 = mock_info("user1", &[]);
        let msg = ExecuteMsg::SetValue {
            name: Some("test1".to_string()),
            value: Primitive::String("value1".to_string()),
        };
        let res: Result<Response, ContractError> = execute(deps.as_mut(), mock_env(), user1, msg);
        assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
    }

    #[test]
    fn non_creator_cannot_delete_value() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg { operators: vec![] };
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let msg = ExecuteMsg::SetValue {
            name: None,
            value: Primitive::String("value1".to_string()),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let user1 = mock_info("user1", &[]);
        let msg = ExecuteMsg::DeleteValue { name: None };
        let res: Result<Response, ContractError> = execute(deps.as_mut(), mock_env(), user1, msg);
        assert_eq!(ContractError::Unauthorized {}, res.unwrap_err());
    }

    #[test]
    fn test_execute_receive() {
        let mut deps = mock_dependencies(&[]);

        let msg = InstantiateMsg { operators: vec![] };
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let msg = ExecuteMsg::SetValue {
            name: None,
            value: Primitive::String("value1".to_string()),
        };
        let msg_binary = encode_binary(&msg).unwrap();
        let msg = ExecuteMsg::AndrReceive(AndromedaMsg::Receive(Some(msg_binary)));
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(
            Response::new()
                .add_attribute("method", "set_value")
                .add_attribute("sender", "creator")
                .add_attribute("name", DEFAULT_KEY)
                .add_attribute("value", "String(\"value1\")"),
            res
        );

        // Try with using the default key.
        let msg = QueryMsg::AndrQuery(AndromedaQuery::Get(None));
        let res: GetValueResponse =
            from_binary(&query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();
        assert_eq!(
            GetValueResponse {
                name: DEFAULT_KEY.to_string(),
                value: Primitive::String("value1".to_string())
            },
            res
        );

        // Try with specifying the key.
        let res = query_value_helper(deps.as_ref(), Some("default".to_string())).unwrap();
        assert_eq!(
            GetValueResponse {
                name: DEFAULT_KEY.to_string(),
                value: Primitive::String("value1".to_string())
            },
            res
        );

        // Try querying by not providing any binary.
        let res = query_value_helper(deps.as_ref(), None).unwrap();
        assert_eq!(
            GetValueResponse {
                name: DEFAULT_KEY.to_string(),
                value: Primitive::String("value1".to_string())
            },
            res
        );
    }
}
