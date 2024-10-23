use andromeda_std::{
    amp::AndrAddr,
    error::ContractError,
    os::vfs::{validate_path_name, SubDirBound, SubSystemBound},
};
use cosmwasm_std::{ensure, Addr, Api, Storage};
use cw_storage_plus::{Bound, Index, IndexList, IndexedMap, Map, MultiIndex};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct PathInfo {
    pub name: String,
    pub address: Addr,
    pub parent_address: Addr,
    pub symlink: Option<AndrAddr>,
}

pub struct PathIndices<'a> {
    /// PK: parent_address + component_name
    /// Secondary key: address
    pub address: MultiIndex<'a, Addr, PathInfo, (Addr, String)>,

    /// PK: parent_address + component_name
    /// Secondary key: parent_address
    pub parent: MultiIndex<'a, Addr, PathInfo, (Addr, String)>,
}

impl<'a> IndexList<PathInfo> for PathIndices<'a> {
    fn get_indexes(
        &'_ self,
    ) -> Box<dyn Iterator<Item = &'_ dyn cw_storage_plus::Index<PathInfo>> + '_> {
        let v: Vec<&dyn Index<PathInfo>> = vec![&self.address, &self.parent];
        Box::new(v.into_iter())
    }
}

pub fn paths<'a>() -> IndexedMap<'a, &'a (Addr, String), PathInfo, PathIndices<'a>> {
    let indexes = PathIndices {
        address: MultiIndex::new(|_pk: &[u8], r| r.address.clone(), "path", "path_index"),
        parent: MultiIndex::new(
            |_pk: &[u8], r| r.parent_address.clone(),
            "path",
            "parent_index",
        ),
    };
    IndexedMap::new("path", indexes)
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct SystemAdoPathInfo {
    pub name: String,
    pub address: Addr,
    pub root: String,
    pub symlink: Option<AndrAddr>,
}

pub struct SystemAdoPathIndices<'a> {
    /// PK: root + system_ado_name
    /// Secondary key: address
    pub address: MultiIndex<'a, Addr, SystemAdoPathInfo, (String, String)>,

    /// PK: root + system_ado_name
    /// Secondary key: root
    pub root: MultiIndex<'a, String, SystemAdoPathInfo, (String, String)>,
}

impl<'a> IndexList<SystemAdoPathInfo> for SystemAdoPathIndices<'a> {
    fn get_indexes(
        &'_ self,
    ) -> Box<dyn Iterator<Item = &'_ dyn cw_storage_plus::Index<SystemAdoPathInfo>> + '_> {
        let v: Vec<&dyn Index<SystemAdoPathInfo>> = vec![&self.address, &self.root];
        Box::new(v.into_iter())
    }
}

pub fn system_ado_paths<'a>(
) -> IndexedMap<'a, &'a (String, String), SystemAdoPathInfo, SystemAdoPathIndices<'a>> {
    let indexes = SystemAdoPathIndices {
        address: MultiIndex::new(
            |_pk: &[u8], r| r.address.clone(),
            "system_ado_path",
            "path_index",
        ),
        root: MultiIndex::new(
            |_pk: &[u8], r| r.root.clone(),
            "system_ado_path",
            "root_index",
        ),
    };
    IndexedMap::new("system_ado_path", indexes)
}

pub const USERS: Map<&str, Addr> = Map::new("users");
pub const LIBRARIES: Map<&str, Addr> = Map::new("libraries");
pub const ADDRESS_USERNAME: Map<&str, String> = Map::new("address_username");
pub const ADDRESS_LIBRARY: Map<&str, String> = Map::new("address_library");

/**
   Splits a pathname into its components.

    * **path**: The full path to be split
*/
pub fn split_pathname(path: String) -> Vec<String> {
    path.split('/')
        .filter(|string| !string.is_empty())
        .map(|string| string.to_string())
        .collect::<Vec<String>>()
}

/**
   Resolves a given path to an address.

    * **storage**: CosmWasm storage struct
    * **api**: CosmWasm API struct
    * **path**: The full path to be resolved
    * **resolved_paths**: A vector of resolved paths to prevent looping or paths that are too long
*/
pub fn resolve_pathname(
    storage: &dyn Storage,
    api: &dyn Api,
    pathname: AndrAddr,
    resolved_paths: &mut Vec<(Addr, String)>,
) -> Result<Addr, ContractError> {
    let pathname = pathname.to_lowercase();
    // As cross-chain queries are not currently possible we need to ensure the pathname being resolved is local
    ensure!(
        pathname.get_protocol().is_none(),
        ContractError::InvalidPathname {
            error: Some("Cannot resolve paths with protocols at this time".to_string())
        }
    );

    if pathname.is_vfs_path() {
        match pathname.get_root_dir() {
            "home" => resolve_home_path(storage, api, pathname, resolved_paths),
            "lib" => resolve_lib_path(storage, api, pathname, resolved_paths),
            // "chain" => resolve_system_path(storage, api, pathname, resolved_paths),
            // &_ => Err(ContractError::InvalidAddress {}),
            &_ => resolve_system_path(storage, api, pathname, resolved_paths),
        }
    } else {
        Ok(api.addr_validate(pathname.as_str())?)
    }
}

/**
   Resolves a given home path.

    * **storage**: CosmWasm storage struct
    * **api**: CosmWasm API struct
    * **path**: The full path to be resolved
    * **resolved_paths**: A vector of resolved paths to prevent looping or paths that are too long
*/
fn resolve_home_path(
    storage: &dyn Storage,
    api: &dyn Api,
    path: AndrAddr,
    resolved_paths: &mut Vec<(Addr, String)>,
) -> Result<Addr, ContractError> {
    validate_path_name(api, path.to_string())?;
    let parts = split_pathname(path.to_string());

    let amount_to_skip = if parts[0].starts_with('~') { 0 } else { 1 };
    let username_or_address = parts[amount_to_skip]
        .strip_prefix('~')
        .unwrap_or(&parts[amount_to_skip]);
    let user_address = match api.addr_validate(username_or_address) {
        Ok(addr) => addr,
        Err(_e) => USERS.load(storage, username_or_address)?,
    };

    let mut remaining_parts = parts.to_vec();

    remaining_parts.drain(0..amount_to_skip + 1);
    resolve_path(storage, api, remaining_parts, user_address, resolved_paths)
}

/**
   Resolves a given library path.

    * **storage**: CosmWasm storage struct
    * **api**: CosmWasm API struct
    * **path**: The full path to be resolved
    * **resolved_paths**: A vector of resolved paths to prevent looping or paths that are too long
*/
fn resolve_lib_path(
    storage: &dyn Storage,
    api: &dyn Api,
    path: AndrAddr,
    resolved_paths: &mut Vec<(Addr, String)>,
) -> Result<Addr, ContractError> {
    let parts = split_pathname(path.to_string());

    let library_or_address = parts[1].as_str();

    let lib_address = match api.addr_validate(library_or_address) {
        Ok(addr) => addr,
        Err(_e) => LIBRARIES.load(storage, library_or_address)?,
    };
    let mut remaining_parts = parts.to_vec();
    remaining_parts.drain(0..2);
    resolve_path(storage, api, remaining_parts, lib_address, resolved_paths)
}

/**
   Resolves a given system path.

    * **storage**: CosmWasm storage struct
    * **api**: CosmWasm API struct
    * **path**: The full path to be resolved
    * **resolved_paths**: A vector of resolved paths to prevent looping or paths that are too long
*/
fn resolve_system_path(
    storage: &dyn Storage,
    api: &dyn Api,
    path: AndrAddr,
    resolved_paths: &mut Vec<(Addr, String)>,
) -> Result<Addr, ContractError> {
    let parts = split_pathname(path.to_string());

    let root = path.get_root_dir();
    let system_ado_name = parts[1].as_str();

    let info =
        system_ado_paths().load(storage, &(root.to_string(), system_ado_name.to_string()))?;

    let address = match info.symlink {
        Some(symlink) => resolve_pathname(storage, api, symlink, resolved_paths)?,
        None => info.address,
    };

    Ok(address)
}

const MAX_DEPTH: u8 = 50;

/**
   Resolves a given path after the first section has been resolved.

   Iterates through the path and resolves each section until the final section is resolved before returning the address.

    * **storage**: CosmWasm storage struct
    * **api**: CosmWasm API struct
    * **parts**: The remaining parts of the path to resolve
    * **parent_address**: The address of the parent lib/user
    * **resolved_paths**: A vector of resolved paths to prevent looping or paths that are too long
*/
fn resolve_path(
    storage: &dyn Storage,
    api: &dyn Api,
    parts: Vec<String>,
    parent_address: Addr,
    resolved_paths: &mut Vec<(Addr, String)>,
) -> Result<Addr, ContractError> {
    let mut address = parent_address;
    // Preemptive length check to prevent resolving paths that are too long
    ensure!(
        parts.len() as u8 <= MAX_DEPTH,
        ContractError::InvalidAddress {}
    );

    for part in parts.iter() {
        // To prevent resolving paths that are too long
        ensure!(
            resolved_paths.len() as u8 <= MAX_DEPTH,
            ContractError::InvalidAddress {}
        );
        // Prevent looping
        ensure!(
            !resolved_paths.contains(&(address.clone(), part.clone())),
            ContractError::InvalidPathname {
                error: Some("Pathname contains a looping reference".to_string())
            }
        );
        let info = paths().load(storage, &(address.clone(), part.clone()))?;
        resolved_paths.push((address, part.clone()));
        address = match info.symlink {
            Some(symlink) => resolve_pathname(storage, api, symlink, resolved_paths)?,
            None => info.address,
        };
    }

    Ok(address)
}

const MAX_LIMIT: u32 = 100u32;
const DEFAULT_LIMIT: u32 = 50u32;

pub fn get_subdir(
    storage: &dyn Storage,
    api: &dyn Api,
    pathname: AndrAddr,
    min: Option<SubDirBound>,
    max: Option<SubDirBound>,
    limit: Option<u32>,
) -> Result<Vec<PathInfo>, ContractError> {
    let address = resolve_pathname(storage, api, pathname, &mut vec![])?;
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

    let subdirs = paths()
        .idx
        .parent
        .prefix(address)
        .range(
            storage,
            min.map(Bound::inclusive),
            max.map(Bound::inclusive),
            cosmwasm_std::Order::Ascending,
        )
        .take(limit as usize)
        .map(|r| r.unwrap().1)
        .collect();

    Ok(subdirs)
}

pub fn get_subsystem(
    storage: &dyn Storage,
    root: String,
    min: Option<SubSystemBound>,
    max: Option<SubSystemBound>,
    limit: Option<u32>,
) -> Result<Vec<SystemAdoPathInfo>, ContractError> {
    // let address = resolve_pathname(storage, api, root.clone(), &mut vec![])?;
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

    let subsystems = system_ado_paths()
        .idx
        .root
        .prefix(root.to_string())
        .range(
            storage,
            min.map(Bound::inclusive),
            max.map(Bound::inclusive),
            cosmwasm_std::Order::Ascending,
        )
        .take(limit as usize)
        .map(|r| r.unwrap().1)
        .collect();

    Ok(subsystems)
}

pub fn get_paths(storage: &dyn Storage, addr: Addr) -> Result<Vec<String>, ContractError> {
    let mut resolved_paths: Vec<String> = vec![];
    let parent_dirs: Vec<PathInfo> = paths()
        .idx
        .address
        .prefix(addr.clone())
        .range(storage, None, None, cosmwasm_std::Order::Ascending)
        .map(|r| r.unwrap().1)
        .collect();
    if parent_dirs.is_empty() {
        // Its a user address
        let username_or_address = ADDRESS_USERNAME
            .load(storage, addr.to_string().as_str())
            .unwrap_or(addr.to_string());
        resolved_paths.push(username_or_address)
    }
    for parent_dir in parent_dirs {
        let parent_paths = get_paths(storage, parent_dir.clone().parent_address)?;
        for parent_path in parent_paths {
            resolved_paths.push(parent_path + "/" + parent_dir.name.as_str());
        }
    }

    Ok(resolved_paths)
}

pub fn get_system_paths(storage: &dyn Storage, addr: Addr) -> Result<Vec<String>, ContractError> {
    let mut resolved_paths: Vec<String> = vec![];
    let all_info: Vec<SystemAdoPathInfo> = system_ado_paths()
        .idx
        .address
        .prefix(addr.clone())
        .range(storage, None, None, cosmwasm_std::Order::Ascending)
        .map(|r| r.unwrap().1)
        .collect();
    for info in all_info {
        let root = info.root;
        let name = info.name;
        resolved_paths.push(format!("/{root}/{name}"));
    }

    Ok(resolved_paths)
}

pub fn add_pathname(
    storage: &mut dyn Storage,
    parent_addr: Addr,
    name: String,
    address: Addr,
) -> Result<(), ContractError> {
    paths().save(
        storage,
        &(parent_addr.clone(), name.clone()),
        &PathInfo {
            name,
            address,
            parent_address: parent_addr,
            symlink: None,
        },
    )?;
    Ok(())
}

pub fn add_system_ado_path_name(
    storage: &mut dyn Storage,
    root: String,
    name: String,
    address: Addr,
) -> Result<(), ContractError> {
    system_ado_paths().save(
        storage,
        &(root.clone(), name.clone()),
        &SystemAdoPathInfo {
            name,
            address,
            root,
            symlink: None,
        },
    )?;
    Ok(())
}

pub fn add_path_symlink(
    storage: &mut dyn Storage,
    api: &dyn Api,
    parent_addr: Addr,
    name: String,
    symlink: AndrAddr,
) -> Result<(), ContractError> {
    paths().save(
        storage,
        &(parent_addr.clone(), name.clone()),
        &PathInfo {
            name: name.clone(),
            address: Addr::unchecked("invalidaddress"),
            parent_address: parent_addr.clone(),
            symlink: Some(symlink.clone()),
        },
    )?;
    if symlink.get_protocol().is_none() {
        // Ensure that the symlink resolves to a valid address
        let pathname = AndrAddr::from_string(format!("~{}/{}", parent_addr, name));
        resolve_pathname(storage, api, pathname, &mut vec![])?;
    }

    Ok(())
}

pub fn resolve_symlink(
    storage: &dyn Storage,
    api: &dyn Api,
    path: AndrAddr,
) -> Result<AndrAddr, ContractError> {
    let mut parts = split_pathname(path.to_string());
    if !path.is_vfs_path() || path.get_protocol().is_some() || parts.len() <= 2 {
        return Ok(path);
    }
    let final_part = parts.pop().unwrap();
    let reconstructed_addr = parts.join("/");
    // Need to prepend a '/' unless the path starts with '~'
    let remaining_path = if reconstructed_addr.starts_with('~') {
        AndrAddr::from_string(reconstructed_addr)
    } else {
        AndrAddr::from_string(format!("/{reconstructed_addr}"))
    };
    let addr = resolve_pathname(storage, api, remaining_path, &mut vec![])?;
    let info = paths().load(storage, &(addr, final_part))?;
    match info.symlink {
        Some(symlink) => Ok(symlink),
        None => Ok(path),
    }
}

#[cfg(test)]
mod test {
    use cosmwasm_std::{testing::mock_dependencies, DepsMut};

    use super::*;

    #[test]
    fn test_split_pathname() {
        let pathname = "//username/dir1/dir2/file";

        let res = split_pathname(pathname.to_string());
        let expected = vec!["username", "dir1", "dir2", "file"];

        assert_eq!(res, expected)
    }

    #[test]
    fn test_resolve_pathname() {
        let path = AndrAddr::from_string("cosmos1...");
        let res = resolve_pathname(
            &mock_dependencies().storage,
            &mock_dependencies().api,
            path,
            &mut vec![],
        )
        .unwrap();
        assert_eq!(res, Addr::unchecked("cosmos1..."));
    }

    #[test]
    fn test_resolve_home_path() {
        let mut deps = mock_dependencies();
        let username = "u1";
        let first_directory = "d1";
        let second_directory = "d2";
        let file = "f1";

        let username_address = Addr::unchecked("useraddress");
        let first_directory_address = Addr::unchecked("dir1address");
        let second_directory_address = Addr::unchecked("dir2address");
        let file_address = Addr::unchecked("fileaddress");

        USERS
            .save(deps.as_mut().storage, username, &username_address)
            .unwrap();

        let res = resolve_home_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!("/home/{username}")),
            &mut vec![],
        )
        .unwrap();
        assert_eq!(res, username_address);

        let res = resolve_home_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!("~{username}")),
            &mut vec![],
        )
        .unwrap();
        assert_eq!(res, username_address);

        let res = resolve_home_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!("~/{username}")),
            &mut vec![],
        );
        assert!(res.is_err());

        paths()
            .save(
                deps.as_mut().storage,
                &(username_address.clone(), first_directory.to_string()),
                &PathInfo {
                    name: first_directory.to_string(),
                    address: first_directory_address.clone(),
                    parent_address: username_address,
                    symlink: None,
                },
            )
            .unwrap();

        let res = resolve_home_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!("/home/{username}/{first_directory}")),
            &mut vec![],
        )
        .unwrap();
        assert_eq!(res, first_directory_address);
        let res = resolve_home_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!("~{username}/{first_directory}")),
            &mut vec![],
        )
        .unwrap();
        assert_eq!(res, first_directory_address);

        paths()
            .save(
                deps.as_mut().storage,
                &(
                    first_directory_address.clone(),
                    second_directory.to_string(),
                ),
                &PathInfo {
                    name: second_directory.to_string(),
                    address: second_directory_address.clone(),
                    parent_address: first_directory_address,
                    symlink: None,
                },
            )
            .unwrap();

        let res = resolve_home_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!(
                "/home/{username}/{first_directory}/{second_directory}"
            )),
            &mut vec![],
        )
        .unwrap();
        assert_eq!(res, second_directory_address);

        paths()
            .save(
                deps.as_mut().storage,
                &(second_directory_address.clone(), file.to_string()),
                &PathInfo {
                    name: file.to_string(),
                    address: file_address.clone(),
                    parent_address: second_directory_address,
                    symlink: None,
                },
            )
            .unwrap();

        let res = resolve_home_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!(
                "/home/{username}/{first_directory}/{second_directory}/{file}"
            )),
            &mut vec![],
        )
        .unwrap();
        assert_eq!(res, file_address)
    }

    #[test]
    fn test_resolve_lib_path() {
        let mut deps = mock_dependencies();
        let lib_name = "l1";
        let first_directory = "d1";
        let second_directory = "d2";
        let file = "f1";

        let username_address = Addr::unchecked("useraddress");
        let first_directory_address = Addr::unchecked("dir1address");
        let second_directory_address = Addr::unchecked("dir2address");
        let file_address = Addr::unchecked("fileaddress");

        LIBRARIES
            .save(deps.as_mut().storage, lib_name, &username_address)
            .unwrap();

        let res = resolve_lib_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!("/lib/{lib_name}")),
            &mut vec![],
        )
        .unwrap();
        assert_eq!(res, username_address);

        paths()
            .save(
                deps.as_mut().storage,
                &(username_address.clone(), first_directory.to_string()),
                &PathInfo {
                    name: first_directory.to_string(),
                    address: first_directory_address.clone(),
                    parent_address: username_address,
                    symlink: None,
                },
            )
            .unwrap();

        let res = resolve_lib_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!("/lib/{lib_name}/{first_directory}")),
            &mut vec![],
        )
        .unwrap();
        assert_eq!(res, first_directory_address);

        paths()
            .save(
                deps.as_mut().storage,
                &(
                    first_directory_address.clone(),
                    second_directory.to_string(),
                ),
                &PathInfo {
                    name: second_directory.to_string(),
                    address: second_directory_address.clone(),
                    parent_address: first_directory_address,
                    symlink: None,
                },
            )
            .unwrap();

        let res = resolve_lib_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!(
                "/lib/{lib_name}/{first_directory}/{second_directory}"
            )),
            &mut vec![],
        )
        .unwrap();
        assert_eq!(res, second_directory_address);

        paths()
            .save(
                deps.as_mut().storage,
                &(second_directory_address.clone(), file.to_string()),
                &PathInfo {
                    name: file.to_string(),
                    address: file_address.clone(),
                    parent_address: second_directory_address,
                    symlink: None,
                },
            )
            .unwrap();

        let res = resolve_lib_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!(
                "/lib/{lib_name}/{first_directory}/{second_directory}/{file}"
            )),
            &mut vec![],
        )
        .unwrap();
        assert_eq!(res, file_address)
    }

    #[test]
    fn test_resolve_system_path() {
        let mut deps = mock_dependencies();
        let root = "etc";
        let system_ado_name = "aos_version";
        let system_ado_address = Addr::unchecked("systemadoaddress");

        system_ado_paths()
            .save(
                deps.as_mut().storage,
                &(root.to_string(), system_ado_name.to_string()),
                &SystemAdoPathInfo {
                    name: system_ado_name.to_string(),
                    address: system_ado_address.clone(),
                    root: root.to_string(),
                    symlink: None,
                },
            )
            .unwrap();

        let res = resolve_system_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!("/{root}/{system_ado_name}")),
            &mut vec![],
        )
        .unwrap();
        assert_eq!(res, system_ado_address);
    }

    #[test]
    fn test_resolve_symlink() {
        let mut deps = mock_dependencies();
        let username = "u1";
        let first_directory = "d1";

        let username_address = Addr::unchecked("useraddress");
        let first_directory_address = Addr::unchecked("dir1address");

        USERS
            .save(deps.as_mut().storage, username, &username_address)
            .unwrap();

        paths()
            .save(
                deps.as_mut().storage,
                &(username_address.clone(), first_directory.to_string()),
                &PathInfo {
                    name: first_directory.to_string(),
                    address: first_directory_address.clone(),
                    parent_address: username_address,
                    symlink: None,
                },
            )
            .unwrap();

        let res = resolve_home_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!("/home/{username}/{first_directory}")),
            &mut vec![],
        )
        .unwrap();
        assert_eq!(res, first_directory_address);

        let symlink_parent = Addr::unchecked("parentaddress");
        let symlink_name = "symlink";
        let symlink = AndrAddr::from_string(format!("/home/{username}/{first_directory}"));
        let DepsMut { api, storage, .. } = deps.as_mut();
        add_path_symlink(
            storage,
            api,
            symlink_parent.clone(),
            symlink_name.to_string(),
            symlink.clone(),
        )
        .unwrap();

        let res = resolve_home_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!("/home/{symlink_parent}/{symlink_name}")),
            &mut vec![],
        )
        .unwrap();
        assert_eq!(res, first_directory_address);

        let res = resolve_symlink(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!("/home/{symlink_parent}/{symlink_name}")),
        )
        .unwrap();

        assert_eq!(res, symlink);

        let res = resolve_symlink(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!("/home/{username}/{first_directory}")),
        )
        .unwrap();

        assert_eq!(
            res,
            AndrAddr::from_string(format!("/home/{username}/{first_directory}"))
        );

        let res = resolve_symlink(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(format!("ibc://chain/home/{symlink_parent}/{symlink}")),
        )
        .unwrap();

        assert_eq!(
            res,
            AndrAddr::from_string(format!("ibc://chain/home/{symlink_parent}/{symlink}"))
        );

        let res = resolve_symlink(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string("someaddress"),
        )
        .unwrap();

        assert_eq!(res, AndrAddr::from_string("someaddress"));

        let res = resolve_symlink(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string("/home/someuser"),
        )
        .unwrap();

        assert_eq!(res, AndrAddr::from_string("/home/someuser"));
    }

    #[test]
    fn test_resolve_path_too_long() {
        let mut deps = mock_dependencies();
        let mut path = "~u1".to_owned();

        for i in 0..MAX_DEPTH + 1 {
            path.push_str(format!("/d{}", i).as_str());
        }

        USERS
            .save(deps.as_mut().storage, "u1", &Addr::unchecked("u1"))
            .unwrap();

        let res = resolve_home_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(path),
            &mut vec![],
        );

        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), ContractError::InvalidAddress {});

        let mut deps = mock_dependencies();
        let mut path: String = "/lib/u1".to_owned();

        for i in 0..MAX_DEPTH + 1 {
            path.push_str(format!("/d{}", i).as_str());
        }

        LIBRARIES
            .save(deps.as_mut().storage, "u1", &Addr::unchecked("u1"))
            .unwrap();

        let res = resolve_lib_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(path),
            &mut vec![],
        );

        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), ContractError::InvalidAddress {})
    }

    #[test]
    fn test_resolve_path_loop() {
        let mut deps = mock_dependencies();
        let path = "~u1/d0".to_owned();

        USERS
            .save(deps.as_mut().storage, "u1", &Addr::unchecked("u1"))
            .unwrap();
        paths()
            .save(
                deps.as_mut().storage,
                &(Addr::unchecked("u1"), "d0".to_string()),
                &PathInfo {
                    name: "d0".to_string(),
                    address: Addr::unchecked("d0"),
                    parent_address: Addr::unchecked("u1"),
                    symlink: None,
                },
            )
            .unwrap();

        let res = resolve_home_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(path.clone()),
            &mut vec![],
        );

        assert!(res.is_ok());

        paths()
            .save(
                deps.as_mut().storage,
                &(Addr::unchecked("d0"), "d1".to_string()),
                &PathInfo {
                    name: "d0".to_string(),
                    address: Addr::unchecked("u1"),
                    parent_address: Addr::unchecked("d1"),
                    symlink: None,
                },
            )
            .unwrap();

        let new_path = format!("{}/d1/d0", path);
        let res = resolve_home_path(
            deps.as_ref().storage,
            deps.as_ref().api,
            AndrAddr::from_string(new_path),
            &mut vec![],
        );

        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            ContractError::InvalidPathname {
                error: Some("Pathname contains a looping reference".to_string())
            }
        );
    }

    #[test]
    fn test_add_symlink_looping_reference() {
        let mut deps = mock_dependencies();
        let username = "u1";
        let first_directory = "d1";

        let username_address = Addr::unchecked("useraddress");
        let first_directory_address = Addr::unchecked("dir1address");

        USERS
            .save(deps.as_mut().storage, username, &username_address)
            .unwrap();

        let DepsMut { api, storage, .. } = deps.as_mut();
        add_pathname(
            storage,
            username_address,
            first_directory.to_string(),
            first_directory_address.clone(),
        )
        .unwrap();

        let res = add_path_symlink(
            storage,
            api,
            first_directory_address,
            username.to_string(),
            AndrAddr::from_string(format!("/home/{username}/{first_directory}/{username}")),
        );
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            ContractError::InvalidPathname {
                error: Some("Pathname contains a looping reference".to_string())
            }
        )
    }
}
