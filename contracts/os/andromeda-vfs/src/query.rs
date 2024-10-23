use andromeda_std::os::vfs::{validate_path_name, SubDirBound, SubSystemBound};
use andromeda_std::{amp::AndrAddr, error::ContractError};
use cosmwasm_std::{Addr, Deps};

use crate::state::{
    get_paths, get_subdir, get_subsystem, get_system_paths, resolve_pathname, resolve_symlink,
    PathInfo, SystemAdoPathInfo, ADDRESS_LIBRARY, ADDRESS_USERNAME,
};

pub fn resolve_path(deps: Deps, path: AndrAddr) -> Result<Addr, ContractError> {
    validate_path_name(deps.api, path.to_string())?;
    resolve_pathname(deps.storage, deps.api, path, &mut vec![])
}
pub fn subdir(
    deps: Deps,
    path: AndrAddr,
    min: Option<SubDirBound>,
    max: Option<SubDirBound>,
    limit: Option<u32>,
) -> Result<Vec<PathInfo>, ContractError> {
    validate_path_name(deps.api, path.to_string())?;
    get_subdir(deps.storage, deps.api, path, min, max, limit)
}

pub fn subsystem(
    deps: Deps,
    root: String,
    min: Option<SubSystemBound>,
    max: Option<SubSystemBound>,
    limit: Option<u32>,
) -> Result<Vec<SystemAdoPathInfo>, ContractError> {
    get_subsystem(deps.storage, root, min, max, limit)
}

pub fn paths(deps: Deps, addr: Addr) -> Result<Vec<String>, ContractError> {
    get_paths(deps.storage, addr)
}

pub fn system_paths(deps: Deps, addr: Addr) -> Result<Vec<String>, ContractError> {
    get_system_paths(deps.storage, addr)
}

pub fn get_symlink(deps: Deps, addr: AndrAddr) -> Result<AndrAddr, ContractError> {
    resolve_symlink(deps.storage, deps.api, addr)
}

pub fn get_username(deps: Deps, addr: Addr) -> Result<String, ContractError> {
    let username = ADDRESS_USERNAME
        .may_load(deps.storage, addr.to_string().as_str())?
        .unwrap_or(addr.to_string());
    Ok(username)
}

pub fn get_library_name(deps: Deps, addr: Addr) -> Result<String, ContractError> {
    let lib_name = ADDRESS_LIBRARY
        .may_load(deps.storage, addr.to_string().as_str())?
        .unwrap_or(addr.to_string());
    Ok(lib_name)
}
