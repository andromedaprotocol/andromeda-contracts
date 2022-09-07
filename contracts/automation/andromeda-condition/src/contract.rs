use ado_base::state::ADOContract;
use andromeda_automation::condition::{
    ExecuteMsg, InstantiateMsg, LogicGate, MigrateMsg, QueryMsg,
};

use common::{
    ado_base::InstantiateMsg as BaseInstantiateMsg, app::AndrAddress, encode_binary,
    error::ContractError, require,
};
use cosmwasm_std::{
    ensure, entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError,
};
use cw2::{get_contract_version, set_contract_version};
use cw_utils::nonpayable;
use semver::Version;

use crate::state::{LOGIC_GATE, RESULTS, WHITELIST};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-evaluation";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    LOGIC_GATE.save(deps.storage, &msg.logic_gate)?;
    WHITELIST.save(deps.storage, &msg.whitelist)?;

    ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        info,
        BaseInstantiateMsg {
            ado_type: "evaluation".to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            operators: None,
            modules: None,
            primitive_contract: None,
        },
    )
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
        ExecuteMsg::Interpret {} => execute_interpret(deps, env, info),
        ExecuteMsg::StoreResult { result } => execute_store_result(deps, env, info, result),
    }
}

fn execute_store_result(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    result: bool,
) -> Result<Response, ContractError> {
    let whitelist = WHITELIST.load(deps.storage)?;
    let contract = ADOContract::default();
    let app_contract = contract.get_app_contract(deps.storage)?;
    // Check authority
    ensure!(
        whitelist.iter().any(
            |x| x.get_address(deps.api, &deps.querier, app_contract.to_owned())
                == Ok(info.sender.to_string())
        ),
        ContractError::Unauthorized {}
    );

    let mut results = RESULTS.load(deps.storage)?;
    results.push(result);
    RESULTS.save(deps.storage, &results)?;
    let whitelist = WHITELIST.load(deps.storage)?;
    // if the number of results equals the number of whitelisted addressses,
    if results.len() == whitelist.len() {
        execute_interpret(deps, _env, info)?;
    }
    Ok(Response::new().add_attribute("action", "stored result"))
}

fn execute_interpret(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    // Check authority
    let contract = ADOContract::default();
    ensure!(
        contract.is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    // Load logic gate
    let logic = LOGIC_GATE.load(deps.storage)?;
    // Load results
    let res = RESULTS.load(deps.storage)?;
    match logic {
        LogicGate::AND =>
        // We don't want to find a false bool, so we want it to return false
        {
            ensure!(
                !res.iter().any(|x| x == &false),
                ContractError::UnmetCondition {}
            );
            Ok(Response::new().add_attribute("result", "sent by AND".to_string()))
        }
        // Just one result being true meets our condition
        LogicGate::OR => {
            ensure!(
                res.iter().any(|x| x == &true),
                ContractError::UnmetCondition {}
            );
            Ok(Response::new().add_attribute("result", "sent by OR".to_string()))
        }
        // At lease one result should be true, but not all of them
        LogicGate::XOR => {
            ensure!(
                !res.iter()
                    .all(|x| x == &true && res.iter().any(|x| x == &true)),
                ContractError::UnmetCondition {}
            );
            Ok(Response::new().add_attribute("result", "sent by XOR".to_string()))
        }
        // Only takes one input, takes false as true
        LogicGate::NOT => {
            ensure!(
                res.len() == 1 && res[0] == false,
                ContractError::UnmetCondition {}
            );
            Ok(Response::new().add_attribute("result", "sent by NOT".to_string()))
        }
        // Any input is valid unless they're all true
        LogicGate::NAND => {
            ensure!(
                !res.iter().all(|x| x == &true),
                ContractError::UnmetCondition {}
            );
            Ok(Response::new().add_attribute("result", "sent by NAND".to_string()))
        }
        // Input should be all false
        LogicGate::NOR => {
            ensure!(
                res.iter().all(|x| x == &false),
                ContractError::UnmetCondition {}
            );
            Ok(Response::new().add_attribute("result", "sent by NOR".to_string()))
        }
        // Input should be all false or all true
        LogicGate::XNOR => {
            ensure!(
                res.iter().all(|x| x == &false) || res.iter().all(|x| x == &true),
                ContractError::UnmetCondition {}
            );
            Ok(Response::new().add_attribute("result", "sent by XNOR".to_string()))
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // New version
    let version: Version = CONTRACT_VERSION.parse().map_err(from_semver)?;

    // Old version
    let stored = get_contract_version(deps.storage)?;
    let storage_version: Version = stored.version.parse().map_err(from_semver)?;

    let contract = ADOContract::default();

    require(
        stored.contract == CONTRACT_NAME,
        ContractError::CannotMigrate {
            previous_contract: stored.contract,
        },
    )?;

    // New version has to be newer/greater than the old version
    require(
        storage_version < version,
        ContractError::CannotMigrate {
            previous_contract: stored.version,
        },
    )?;

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Update the ADOContract's version
    contract.execute_update_version(deps)?;

    Ok(Response::default())
}

fn from_semver(err: semver::Error) -> StdError {
    StdError::generic_err(format!("Semver: {}", err))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::AndrQuery(msg) => ADOContract::default().query(deps, env, msg, query),
        QueryMsg::LogicGate {} => encode_binary(&query_logic_gate(deps)?),
        QueryMsg::Whitelist {} => encode_binary(&query_whitelist(deps)?),
    }
}

fn query_logic_gate(deps: Deps) -> Result<LogicGate, ContractError> {
    Ok(LOGIC_GATE.load(deps.storage)?)
}

fn query_whitelist(deps: Deps) -> Result<Vec<AndrAddress>, ContractError> {
    Ok(WHITELIST.load(deps.storage)?)
}
