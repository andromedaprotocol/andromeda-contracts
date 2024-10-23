use andromeda_std::ado_contract::ADOContract;
use andromeda_std::amp::AndrAddr;
use andromeda_std::error::ContractError;
use andromeda_std::os::aos_querier::AOSQuerier;
use andromeda_std::os::kernel::InternalMsg;
use andromeda_std::os::{
    kernel::ExecuteMsg as KernelExecuteMsg,
    vfs::{validate_component_name, validate_username},
};
use cosmwasm_std::{
    attr, ensure, to_json_binary, Addr, DepsMut, Env, MessageInfo, Response, SubMsg, WasmMsg,
};

use crate::state::{
    add_path_symlink, add_pathname, add_system_ado_path_name, paths, resolve_pathname,
    system_ado_paths, ADDRESS_LIBRARY, ADDRESS_USERNAME, LIBRARIES, USERS,
};

pub struct ExecuteEnv<'a> {
    pub deps: DepsMut<'a>,
    pub env: Env,
    pub info: MessageInfo,
}

pub fn add_path(
    env: ExecuteEnv,
    name: String,
    address: Addr,
    parent_address: Option<AndrAddr>,
) -> Result<Response, ContractError> {
    let name = name.to_lowercase();
    let kernel_address = ADOContract::default().get_kernel_address(env.deps.storage)?;
    ensure!(
        parent_address.is_none()
            || env.info.sender == kernel_address
            || ADOContract::default()
                .is_contract_owner(env.deps.storage, env.info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    let parent_andr_addr = parent_address.unwrap_or(AndrAddr::from_string(env.info.sender));
    let parent_addr = resolve_pathname(
        env.deps.storage,
        env.deps.api,
        parent_andr_addr,
        &mut vec![],
    )?;
    validate_component_name(name.clone())?;
    add_pathname(
        env.deps.storage,
        parent_addr.clone(),
        name.clone(),
        address.clone(),
    )?;
    Ok(Response::default().add_attributes(vec![
        attr("action", "add_path"),
        attr("addr", address),
        attr("name", name),
        attr("parent", parent_addr),
    ]))
}

pub fn add_symlink(
    env: ExecuteEnv,
    name: String,
    symlink: AndrAddr,
    parent_address: Option<AndrAddr>,
) -> Result<Response, ContractError> {
    let name = name.to_lowercase();
    let symlink = symlink.to_lowercase();
    let kernel_address = ADOContract::default().get_kernel_address(env.deps.storage)?;
    ensure!(
        parent_address.is_none()
            || env.info.sender == kernel_address
            || ADOContract::default()
                .is_contract_owner(env.deps.storage, env.info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    let parent_andr_addr = parent_address.unwrap_or(AndrAddr::from_string(env.info.sender));
    let parent_addr = resolve_pathname(
        env.deps.storage,
        env.deps.api,
        parent_andr_addr,
        &mut vec![],
    )?;
    validate_component_name(name.clone())?;
    add_path_symlink(
        env.deps.storage,
        env.deps.api,
        parent_addr.clone(),
        name.clone(),
        symlink.clone(),
    )?;
    Ok(Response::default().add_attributes(vec![
        attr("action", "add_symlink"),
        attr("symlink", symlink),
        attr("name", name),
        attr("parent", parent_addr),
    ]))
}

pub fn add_child(
    env: ExecuteEnv,
    name: String,
    parent_address: AndrAddr,
) -> Result<Response, ContractError> {
    let ExecuteEnv { deps, info, .. } = env;
    let name = name.to_lowercase();

    let sender_code_id_res = deps.querier.query_wasm_contract_info(info.sender.clone());
    // Sender must be a contract
    ensure!(sender_code_id_res.is_ok(), ContractError::Unauthorized {});
    let sender_code_id = sender_code_id_res?.code_id;
    let ado_type = AOSQuerier::ado_type_getter(
        &deps.querier,
        &ADOContract::default().get_adodb_address(deps.as_ref().storage, &deps.querier)?,
        sender_code_id,
    )?;
    // Sender must be an app contract
    ensure!(
        ado_type.is_some() && ado_type.unwrap().contains("app-contract"),
        ContractError::Unauthorized {}
    );

    validate_component_name(name.clone())?;
    let parent_address = resolve_pathname(deps.storage, deps.api, parent_address, &mut vec![])?;
    let existing = paths()
        .load(deps.storage, &(parent_address.clone(), name.clone()))
        .ok();
    // Ensure that this path is not already added or if already added it should point to same address as above. This prevent external users to override existing paths.
    // Only add path method can override existing paths as its safe because only owner of the path can execute it
    match existing {
        None => {
            add_pathname(deps.storage, parent_address, name, info.sender)?;
        }
        Some(path) => {
            ensure!(path.address == info.sender, ContractError::Unauthorized {})
        }
    };
    Ok(Response::default())
}

pub fn add_system_ado_path(
    env: ExecuteEnv,
    name: String,
    root: String,
) -> Result<Response, ContractError> {
    let ExecuteEnv { deps, info, .. } = env;
    let name = name.to_lowercase();
    let root = root.to_lowercase();

    let sender_code_id_res = deps.querier.query_wasm_contract_info(info.sender.clone());
    // Sender must be an ADO contract
    ensure!(sender_code_id_res.is_ok(), ContractError::Unauthorized {});

    let sender_code_id = sender_code_id_res?.code_id;
    let ado_type = AOSQuerier::ado_type_getter(
        &deps.querier,
        &ADOContract::default().get_adodb_address(deps.as_ref().storage, &deps.querier)?,
        sender_code_id,
    )?;
    // Sender must be an System ADO contract
    ensure!(ado_type.clone().is_some(), ContractError::Unauthorized {});

    validate_component_name(name.clone())?;
    validate_component_name(root.clone())?;

    let existing = system_ado_paths()
        .load(deps.storage, &(root.clone(), name.clone()))
        .ok();
    // Ensure that this path is not already added or if already added it should point to same address as above. This prevent external users to override existing paths.
    // Only add path method can override existing paths as its safe because only owner of the path can execute it
    match existing {
        None => {
            add_system_ado_path_name(deps.storage, root.clone(), name.clone(), info.sender)?;
        }
        Some(path) => {
            ensure!(path.address == info.sender, ContractError::Unauthorized {});
            return Err(ContractError::CustomError {
                msg: "System ADO already exists".to_string(),
            });
        }
    };
    Ok(Response::default().add_attributes(vec![
        attr("action", "add_system_ado_path"),
        attr("name", name.clone()),
        attr("root", root.clone()),
    ]))
}

const MAX_USERNAME_LENGTH: u64 = 30;

pub fn register_user(
    env: ExecuteEnv,
    username: String,
    address: Option<Addr>,
) -> Result<Response, ContractError> {
    #[cfg(not(test))]
    ensure!(false, ContractError::TemporarilyDisabled {});
    ensure!(
        username.len() as u64 <= MAX_USERNAME_LENGTH,
        ContractError::InvalidUsername {
            error: Some(format!(
                "Username must be less than {MAX_USERNAME_LENGTH} characters"
            ))
        }
    );
    let username = username.to_lowercase();
    let kernel = &ADOContract::default().get_kernel_address(env.deps.storage)?;
    let curr_chain = AOSQuerier::get_current_chain(&env.deps.querier, kernel)?;
    // Can only register username directly on Andromeda chain
    ensure!(
        curr_chain == "andromeda" || env.info.sender == kernel,
        ContractError::Unauthorized {}
    );
    // If address is provided sender must be Kernel
    ensure!(
        address.is_none() || env.info.sender == kernel,
        ContractError::Unauthorized {}
    );
    // Kernel must provide an address
    ensure!(
        env.info.sender != kernel || address.is_some(),
        ContractError::Unauthorized {}
    );
    let sender = address.unwrap_or(env.info.sender.clone());
    let current_user_address = USERS.may_load(env.deps.storage, username.as_str())?;
    ensure!(
        current_user_address.is_none(),
        ContractError::InvalidUsername {
            error: Some("Username already taken".to_string())
        }
    );

    //Remove username registration from previous username
    let current_username = ADDRESS_USERNAME.may_load(env.deps.storage, sender.as_ref())?;
    if let Some(current_username) = current_username {
        USERS.remove(env.deps.storage, current_username.as_str());
    }

    // If the username is a valid address, it should be equal to info.sender
    match env.deps.api.addr_validate(&username) {
        Ok(username) => {
            // No need to validate the username any further if this passess
            ensure!(
                username == env.info.sender,
                ContractError::InvalidUsername {
                    error: Some(
                        "Usernames that are valid addresses should be the same as the sender's address"
                            .to_string()
                    )
                }
            )
        }
        Err(_) => {
            validate_username(username.clone())?;
        }
    }

    USERS.save(env.deps.storage, username.as_str(), &sender)?;
    //Update current address' username
    ADDRESS_USERNAME.save(env.deps.storage, sender.as_ref(), &username)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "register_username"),
        attr("addr", sender),
        attr("username", username),
    ]))
}

pub fn register_library(
    env: ExecuteEnv,
    lib_name: String,
    lib_address: Addr,
) -> Result<Response, ContractError> {
    let kernel_address = ADOContract::default().get_kernel_address(env.deps.storage)?;
    ensure!(
        env.info.sender == kernel_address
            || ADOContract::default()
                .is_contract_owner(env.deps.storage, env.info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    validate_username(lib_name.clone())?;
    LIBRARIES.save(env.deps.storage, lib_name.as_str(), &lib_address)?;
    //Update current address' username
    ADDRESS_LIBRARY.save(env.deps.storage, lib_address.as_str(), &lib_name)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "register_library"),
        attr("addr", lib_address),
        attr("library_name", lib_name),
    ]))
}

pub fn register_user_cross_chain(
    env: ExecuteEnv,
    chain: String,
    address: String,
) -> Result<Response, ContractError> {
    let kernel = ADOContract::default().get_kernel_address(env.deps.storage)?;
    let username = ADDRESS_USERNAME.load(env.deps.storage, env.info.sender.as_str())?;
    let msg = KernelExecuteMsg::Internal(InternalMsg::RegisterUserCrossChain {
        username: username.clone(),
        address: address.clone(),
        chain: chain.clone(),
    });
    let sub_msg = SubMsg::reply_on_error(
        WasmMsg::Execute {
            contract_addr: kernel.to_string(),
            msg: to_json_binary(&msg)?,
            funds: vec![],
        },
        1,
    );

    Ok(Response::default()
        .add_attributes(vec![
            attr("action", "register_user_cross_chain"),
            attr("addr", address),
            attr("username", username),
            attr("chain", chain),
        ])
        .add_submessage(sub_msg))
}
