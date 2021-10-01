use cosmwasm_std::{
    entry_point, to_binary, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult,
};

use cw721::Expiration;

use crate::state::{State, STATE};
use andromeda_protocol::{
    require::require,
    timelock::{
        get_funds, hold_funds, release_funds, Escrow, ExecuteMsg, InstantiateMsg, QueryMsg,
    },
};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response> {
    let state = State { owner: info.sender };
    STATE.save(deps.storage, &state)?;
    Ok(Response::default())
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::HoldFunds { expire } => execute_hold_funds(deps, info, expire),
        ExecuteMsg::ReleaseFunds {} => execute_release_funds(deps, env, info),
    }
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetLockedFunds { address } => query_process(deps, address),
    }
}

fn query_process(deps: Deps, address: String) -> StdResult<Binary> {
    let hold_funds = get_funds(deps.storage, address.clone())?;
    match hold_funds {
        None => Err(StdError::generic_err("No locked funds for your account")),
        Some(f) => {
            to_binary(&f) //HoldFunds
        }
    }
}

fn execute_hold_funds(deps: DepsMut, info: MessageInfo, expire: Expiration) -> StdResult<Response> {
    let result: Option<Escrow> = get_funds(deps.storage, info.sender.to_string())?;
    require(
        result.is_none(),
        StdError::generic_err("Funds are already being held for this address"),
    )?;

    let sent_funds: Vec<Coin> = info.funds.clone();
    if sent_funds.len() == 0 {
        return Err(StdError::generic_err("No funds provided"));
    }

    let escrow = Escrow {
        coins: sent_funds,
        expire,
    };
    hold_funds(escrow, deps.storage, info.sender.to_string())?;

    Ok(Response::default())
}

fn execute_release_funds(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let result: Option<Escrow> = get_funds(deps.storage, info.sender.to_string())?; // StdResult<Option<HoldFunds>>

    if result == None {
        return Err(StdError::generic_err("No locked funds for your address"));
    }
    let funds: Escrow = result.unwrap();

    match funds.expire {
        Expiration::AtTime(t) => {
            if t > env.block.time {
                return Err(StdError::generic_err("Your funds are still locked"));
            }
        }
        Expiration::AtHeight(h) => {
            if h > env.block.height {
                return Err(StdError::generic_err("Your funds are still locked"));
            }
        }
        Expiration::Never {} => {}
    }

    release_funds(deps.storage, info.sender.to_string());
    Ok(Response::new().add_message(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: funds.coins,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    #[test]
    fn test_instantiate() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let owner = "owner";
        let info = mock_info(owner, &[]);
        let msg = InstantiateMsg {};
        let res = instantiate(deps.as_mut(), env, info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());
        //checking
        let state = STATE.load(deps.as_ref().storage).unwrap();
        assert!(state.owner == owner);
    }

    #[test]
    fn test_execute_hold_funds() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let owner = "owner";
        let info = mock_info(owner, &vec![Coin::new(1000, "uusd")]);
        let msg = ExecuteMsg::HoldFunds {
            expire: Expiration::Never {},
        };

        //add address for registered moderator

        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        assert_eq!(Response::default(), res);
    }

    #[test]
    fn test_execute_release_funds() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let owner = "owner";

        let info = mock_info(owner, &vec![Coin::new(1000, "uusd")]);
        let msg = ExecuteMsg::HoldFunds {
            expire: Expiration::Never {},
        };

        //add address for registered moderator
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

        let info = mock_info(owner, &[]);
        let msg = ExecuteMsg::ReleaseFunds {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        assert_ne!(Response::default(), res);
    }
}
