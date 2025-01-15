#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure, Binary, Deps, DepsMut, Env, Event, MessageInfo, Reply, Response, StdError, Uint64,
};

use andromeda_data_storage::form::{ExecuteMsg, InstantiateMsg, QueryMsg};
use andromeda_std::{
    ado_base::{
        permissioning::{LocalPermission, Permission},
        InstantiateMsg as BaseInstantiateMsg, MigrateMsg,
    },
    ado_contract::ADOContract,
    common::{
        context::ExecuteContext, encode_binary, expiration::get_and_validate_start_time,
        Milliseconds,
    },
    error::ContractError,
};

use crate::execute::{handle_execute, milliseconds_from_expiration};
use crate::query::{
    get_all_submissions, get_form_status, get_schema, get_submission, get_submission_ids,
};
use crate::state::{Config, CONFIG, SCHEMA_ADO_ADDRESS, SUBMISSION_ID};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-form";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const SUBMIT_FORM_ACTION: &str = "submit_form";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let resp = ADOContract::default().instantiate(
        deps.storage,
        env.clone(),
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

    let schema_ado_address = msg.schema_ado_address;
    schema_ado_address.validate(deps.api)?;

    SCHEMA_ADO_ADDRESS.save(deps.storage, &schema_ado_address)?;
    SUBMISSION_ID.save(deps.storage, &Uint64::zero())?;

    let start_time = match msg.form_config.start_time {
        Some(start_time) => Some(milliseconds_from_expiration(
            get_and_validate_start_time(&env.clone(), Some(start_time))?.0,
        )?),
        None => None,
    };
    let end_time = match msg.form_config.end_time {
        Some(end_time) => {
            let time_res = get_and_validate_start_time(&env.clone(), Some(end_time));
            if time_res.is_ok() {
                Some(milliseconds_from_expiration(time_res.unwrap().0)?)
            } else {
                let current_time = Milliseconds::from_nanos(env.block.time.nanos()).milliseconds();
                let current_height = env.block.height;
                return Err(ContractError::CustomError {
                    msg: format!(
                        "End time in the past. current_time {:?}, current_block {:?}",
                        current_time, current_height
                    ),
                });
            }
        }
        None => None,
    };

    if let (Some(start_time), Some(end_time)) = (start_time, end_time) {
        ensure!(
            end_time.gt(&start_time),
            ContractError::StartTimeAfterEndTime {}
        );
    }

    let allow_multiple_submissions = msg.form_config.allow_multiple_submissions;
    let allow_edit_submission = msg.form_config.allow_edit_submission;

    let config = Config {
        start_time,
        end_time,
        allow_multiple_submissions,
        allow_edit_submission,
    };

    CONFIG.save(deps.storage, &config)?;

    if let Some(authorized_addresses_for_submission) = msg.authorized_addresses_for_submission {
        if !authorized_addresses_for_submission.is_empty() {
            ADOContract::default().permission_action(SUBMIT_FORM_ACTION, deps.storage)?;
        }

        for address in authorized_addresses_for_submission {
            let addr = address.get_raw_address(&deps.as_ref())?;
            ADOContract::set_permission(
                deps.storage,
                SUBMIT_FORM_ACTION,
                addr,
                Permission::Local(LocalPermission::whitelisted(None, None)),
            )?;
        }
    }

    let mut response = resp.add_event(Event::new("form_instantiated"));

    if let Some(custom_key) = msg.custom_key_for_notifications {
        response = response.add_event(
            cosmwasm_std::Event::new("custom_key")
                .add_attribute("custom_key", custom_key)
                .add_attribute("notification_service", "Telegram"),
        );
    }

    Ok(response)
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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetSchema {} => encode_binary(&get_schema(deps)?),
        QueryMsg::GetAllSubmissions {} => encode_binary(&get_all_submissions(deps.storage)?),
        QueryMsg::GetSubmission {
            submission_id,
            wallet_address,
        } => encode_binary(&get_submission(deps, submission_id, wallet_address)?),
        QueryMsg::GetSubmissionIds { wallet_address } => {
            encode_binary(&get_submission_ids(deps, wallet_address)?)
        }
        QueryMsg::GetFormStatus {} => encode_binary(&get_form_status(deps.storage, env)?),
        _ => ADOContract::default().query(deps, env, msg),
    }
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
