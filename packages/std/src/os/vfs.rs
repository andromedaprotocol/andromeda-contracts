use crate::{ado_base::ownership::OwnershipMessage, amp::AndrAddr, error::ContractError};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{ensure, Addr, Api, QuerierWrapper};
use regex::Regex;

pub const COMPONENT_NAME_REGEX: &str = r"^[A-Za-z0-9.\-_]{2,80}$";
pub const USERNAME_REGEX: &str = r"^[a-z0-9]{2,30}$";

pub const PATH_REGEX: &str =
    r"^(~[a-z0-9]{2,}|/(lib|home|[a-z0-9-_]{2,}))(/[A-Za-z0-9.\-_]{2,80}?)*(/)?$";
pub const PROTOCOL_PATH_REGEX: &str = r"^((([A-Za-z0-9]+://)?([A-Za-z0-9.\-_]{2,80}/)))?((~[a-z0-9]{2,}|(lib|home))(/[A-Za-z0-9.\-_]{2,80}?)*(/)?)$";

pub fn convert_component_name(path: &str) -> String {
    path.trim()
        .replace(' ', "_")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect::<String>()
        .to_lowercase()
}

pub fn validate_component_name(path: String) -> Result<bool, ContractError> {
    ensure!(
        path.chars().any(|c| c.is_alphanumeric()),
        ContractError::InvalidPathname {
            error: Some("Pathname must contain at least one alphanumeric character".to_string())
        }
    );
    let re = Regex::new(COMPONENT_NAME_REGEX).unwrap();

    ensure!(
        re.is_match(&path),
        ContractError::InvalidPathname {
            error: Some("Pathname includes an invalid character".to_string())
        }
    );

    Ok(true)
}

/// Validates a username against specific criteria.
///
/// This function checks if a given username meets the following conditions:
/// - It must contain at least three characters
/// - It must only contain alphanumeric characters
///
/// # Arguments
///
/// * `username` - A `String` representing the username to be validated.
///
/// # Returns
///
/// * `Result<bool, ContractError>` - Returns `Ok(true)` if the username is valid, otherwise returns an `Err` with a `ContractError` detailing the reason for invalidity.
pub fn validate_username(username: String) -> Result<bool, ContractError> {
    // Ensure the username is not empty.
    ensure!(
        !username.is_empty(),
        ContractError::InvalidUsername {
            error: Some("Username cannot be empty.".to_string())
        }
    );

    // Compile the regex for validating alphanumeric characters.
    let re = Regex::new(USERNAME_REGEX).unwrap();
    // Ensure the username matches the alphanumeric regex pattern.
    ensure!(
        re.is_match(&username),
        ContractError::InvalidPathname {
            error: Some(
                "Username contains invalid characters. All characters must be alphanumeric."
                    .to_string()
            )
        }
    );
    // Return true if all validations pass.
    Ok(true)
}

pub fn validate_path_name(api: &dyn Api, path: String) -> Result<(), ContractError> {
    let andr_addr = AndrAddr::from_string(path.clone());
    let is_path_reference = path.contains('/');
    let includes_protocol = andr_addr.get_protocol().is_some();

    // Path is of the form /user/... or /lib/... or prot://...
    if is_path_reference {
        // Alter regex if path includes a protocol
        let regex_str = if includes_protocol {
            PROTOCOL_PATH_REGEX
        } else {
            PATH_REGEX
        };

        let re = Regex::new(regex_str).unwrap();
        ensure!(
            re.is_match(&path),
            ContractError::InvalidPathname {
                error: Some("Pathname includes an invalid character".to_string())
            }
        );

        return Ok(());
    }

    // Path is either a username or address
    if !is_path_reference {
        let path = path.strip_prefix('~').unwrap_or(&path);
        let is_address = api.addr_validate(path).is_ok();

        if is_address {
            return Ok(());
        }

        let is_username = validate_username(path.to_string()).is_ok();

        if is_username {
            return Ok(());
        }

        return Err(ContractError::InvalidPathname {
            error: Some(
                "Provided address is neither a valid username nor a valid address".to_string(),
            ),
        });
    }

    // Does not fit any valid conditions
    Err(ContractError::InvalidPathname { error: None })
}

#[cw_serde]
pub struct InstantiateMsg {
    /// Address of the Kernel contract on chain
    pub kernel_address: String,
    pub owner: Option<String>,
}

#[cw_serde]
pub struct PathDetails {
    name: String,
    address: Addr,
}

impl PathDetails {
    pub fn new(name: impl Into<String>, address: Addr) -> PathDetails {
        PathDetails {
            name: name.into(),
            address,
        }
    }
}

#[cw_serde]
pub enum ExecuteMsg {
    AddPath {
        #[schemars(regex = "COMPONENT_NAME_REGEX")]
        name: String,
        address: Addr,
        parent_address: Option<AndrAddr>,
    },
    AddSymlink {
        #[schemars(regex = "COMPONENT_NAME_REGEX")]
        name: String,
        symlink: AndrAddr,
        parent_address: Option<AndrAddr>,
    },
    // Registers a child, currently only accessible by an App Contract
    AddChild {
        #[schemars(regex = "COMPONENT_NAME_REGEX")]
        name: String,
        parent_address: AndrAddr,
    },
    // Registers a system ADO, currently only accessible by a registered admin
    AddSystemAdoPath {
        #[schemars(regex = "COMPONENT_NAME_REGEX")]
        name: String,
        #[schemars(regex = "COMPONENT_NAME_REGEX")]
        root: String,
    },
    RegisterUser {
        #[schemars(regex = "USERNAME_REGEX", length(min = 3, max = 30))]
        username: String,
        address: Option<Addr>,
    },
    // Restricted to VFS owner/Kernel
    RegisterLibrary {
        lib_name: String,
        lib_address: Addr,
    },
    RegisterUserCrossChain {
        chain: String,
        address: String,
    },
    // Base message
    Ownership(OwnershipMessage),
}

#[cw_serde]
pub struct SubDirBound {
    address: Addr,
    name: String,
}
impl From<SubDirBound> for (Addr, String) {
    fn from(val: SubDirBound) -> Self {
        (val.address, val.name)
    }
}

#[cw_serde]
pub struct SubSystemBound {
    root: String,
    name: String,
}
impl From<SubSystemBound> for (String, String) {
    fn from(val: SubSystemBound) -> Self {
        (val.root, val.name)
    }
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Addr)]
    ResolvePath { path: AndrAddr },
    #[returns(Vec<PathDetails>)]
    SubDir {
        path: AndrAddr,
        min: Option<SubDirBound>,
        max: Option<SubDirBound>,
        limit: Option<u32>,
    },
    #[returns(Vec<PathDetails>)]
    SubSystem {
        root: String,
        min: Option<SubSystemBound>,
        max: Option<SubSystemBound>,
        limit: Option<u32>,
    },
    #[returns(Vec<String>)]
    Paths { addr: Addr },
    #[returns(Vec<String>)]
    SystemPaths { addr: Addr },
    #[returns(String)]
    GetUsername { address: Addr },
    #[returns(String)]
    GetLibrary { address: Addr },
    #[returns(AndrAddr)]
    ResolveSymlink { path: AndrAddr },
    // Base queries
    #[returns(crate::ado_base::version::VersionResponse)]
    Version {},
    #[returns(crate::ado_base::ado_type::TypeResponse)]
    Type {},
    #[returns(crate::ado_base::ownership::ContractOwnerResponse)]
    Owner {},
    #[returns(crate::ado_base::kernel_address::KernelAddressResponse)]
    KernelAddress {},
}

/// Queries the provided VFS contract address to resolve the given path
pub fn vfs_resolve_path(
    path: impl Into<String>,
    vfs_contract: impl Into<String>,
    querier: &QuerierWrapper,
) -> Result<Addr, ContractError> {
    let query = QueryMsg::ResolvePath {
        path: AndrAddr::from_string(path.into()),
    };
    let addr = querier.query_wasm_smart::<Addr>(vfs_contract, &query);
    match addr {
        Ok(addr) => Ok(addr),
        Err(_) => Err(ContractError::InvalidAddress {}),
    }
}

/// Queries the provided VFS contract address to resolve the given path
pub fn vfs_resolve_symlink(
    path: impl Into<String>,
    vfs_contract: impl Into<String>,
    querier: &QuerierWrapper,
) -> Result<AndrAddr, ContractError> {
    let query = QueryMsg::ResolveSymlink {
        path: AndrAddr::from_string(path.into()),
    };
    let addr = querier.query_wasm_smart::<AndrAddr>(vfs_contract, &query)?;
    Ok(addr)
}

#[cfg(test)]
mod test {
    use cosmwasm_std::testing::mock_dependencies;

    use super::*;

    struct ValidateComponentNameTestCase {
        name: &'static str,
        input: &'static str,
        should_err: bool,
    }

    #[test]
    fn test_validate_component_name() {
        let test_cases: Vec<ValidateComponentNameTestCase> = vec![
            ValidateComponentNameTestCase {
                name: "standard component name",
                input: "component1",
                should_err: false
            },
            ValidateComponentNameTestCase {
                name: "component with hyphen",
                input: "component-2",
                should_err: false,
            },
            ValidateComponentNameTestCase {
                name: "component with underscore",
                input: "component_2",
                should_err: false,
            },
            ValidateComponentNameTestCase {
                name: "component with period",
                input: ".component2",
                should_err: false,
            },
            ValidateComponentNameTestCase {
                name: "component with invalid character",
                input: "component$2",
                should_err: true,
            },
            ValidateComponentNameTestCase {
                name: "component with spaces",
                input: "component 2",
                should_err: true,
            },
            ValidateComponentNameTestCase {
                name: "empty component name",
                input: "",
                should_err: true,
            },
            ValidateComponentNameTestCase {
                name: "component name too long",
                input: "somereallyreallyreallyreallyreallyreallyreallyreallyreallyreallyreallyreallylongname",
                should_err: true,
            },
            ValidateComponentNameTestCase {
                name: "component name with only special characters",
                input: "!@#$%^&*()",
                should_err: true,
            },
            ValidateComponentNameTestCase {
                name: "component name with leading and trailing spaces",
                input: "  component2  ",
                should_err: true,
            },
            ValidateComponentNameTestCase {
                name: "component name with only numbers",
                input: "123456",
                should_err: false,
            },
            ValidateComponentNameTestCase {
                name: "component name one letter",
                input: "a",
                should_err: true,
            },
            ValidateComponentNameTestCase {
                name: "component name two letters",
                input: "ab",
                should_err: false,
            },
            ValidateComponentNameTestCase {
                name: "component with hyphen at the start",
                input: "-component-2",
                should_err: false,
            },
            ValidateComponentNameTestCase {
                name: "component with forward slash",
                input: "component-2/",
                should_err: true,
            },
            ValidateComponentNameTestCase {
                name: "component with backward slash",
                input: r"component-2\",
                should_err: true,
            },
            ValidateComponentNameTestCase {
                name: "component name with upper case letters",
                input: "ComponentName",
                should_err: false,
            }
        ];

        for test in test_cases {
            let res = validate_component_name(test.input.to_string());
            assert_eq!(res.is_err(), test.should_err, "Test case: {}", test.name);
        }
    }

    struct ValidatePathNameTestCase {
        name: &'static str,
        path: &'static str,
        should_err: bool,
    }

    #[test]
    fn test_validate_path_name() {
        let test_cases: Vec<ValidatePathNameTestCase> = vec![
            ValidatePathNameTestCase {
                name: "System ADO path 1",
                path: "/etc/aos_version",
                should_err: false,
            },
            ValidatePathNameTestCase {
                name: "System ADO path 2",
                path: "/chain/current_block",
                should_err: false,
            },
            ValidatePathNameTestCase {
                name: "Simple app path",
                path: "./username/app",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Root path",
                path: "/",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Relative path with parent directory",
                path: "../username/app",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Tilde username reference",
                // Username must be short to circumvent it being mistaken as an address
                path: "~usr",
                should_err: false,
            },
            ValidatePathNameTestCase {
                name: "Tilde address reference",
                path: "~cosmos1abcde",
                should_err: false,
            },
            ValidatePathNameTestCase {
                name: "Tilde username reference with directory",
                path: "~usr/app/splitter",
                should_err: false,
            },
            ValidatePathNameTestCase {
                name: "Invalid tilde username reference",
                path: "~/username",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Absolute path with tilde",
                path: "~/home/username",
                should_err: true,
            },
            // // It is disable becasue system ado path
            // ValidatePathNameTestCase {
            //     name: "Invalid user path",
            //     path: "/user/un",
            //     should_err: true,
            // },
            ValidatePathNameTestCase {
                name: "Valid user path",
                path: "/home/usr",
                should_err: false,
            },
            // // It is disable becasue system ado path
            // ValidatePathNameTestCase {
            //     name: "Invalid home path (address)",
            //     path: "/user/cosmos1abcde",
            //     should_err: true,
            // },
            ValidatePathNameTestCase {
                name: "Valid home path (address)",
                path: "/home/cosmos1abcde",
                should_err: false,
            },
            ValidatePathNameTestCase {
                name: "Valid lib path",
                path: "/lib/library",
                should_err: false,
            },
            ValidatePathNameTestCase {
                name: "Complex invalid path",
                path: "/home/username/dir1/../dir2/./file",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Path with invalid characters",
                path: "/home/username/dir1/|file",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Path with space",
                path: "/home/ username/dir1/file",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Empty path",
                path: "",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Path with only special characters",
                path: "///",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Path with only special characters and spaces",
                path: "/// /  /// //",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Valid ibc protocol path",
                path: "ibc://chain/home/username/dir1/file",
                should_err: false,
            },
            ValidatePathNameTestCase {
                name: "Invalid ibc protocol path",
                path: "ibc:///home/username/dir1/file",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Standard address",
                path: "cosmos1abcde",
                should_err: false,
            },
            ValidatePathNameTestCase {
                name: "Only periods",
                path: "/../../../..",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Path with newline character",
                path: "/home/username/dir1\n/file",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Path with tab character",
                path: "/home/username/dir1\t/dir2",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Path with null character",
                path: "/home/username\0/dir1",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Path with emoji",
                path: "/home/username/😊",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Path with Cyrillic characters",
                path: "/home/пользователь/dir1",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Path with Arabic characters",
                path: "/home/مستخدم/dir1",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Path with Chinese characters",
                path: "/home/用户/dir1",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Path with very long name",
                path: "/home/username/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                should_err: true,
            },
            ValidatePathNameTestCase {
                name: "Valid path with multiple subdirectories",
                path: "/home/username/dir1/dir2/dir3/dir4",
                should_err: false,
            },
            ValidatePathNameTestCase {
                name: "Path with unprintable ASCII character",
                path: "/home/username/\x07file",
                should_err: true,
            },
            // This case should fail but due to the restriction of mock dependencies we cannot validate it correctly! It is partially validated in test_validate_username
            // ValidatePathNameTestCase {
            //     name: "Really long username",
            //     path: "~somereallyreallyreallyreallyreallyreallyreallyreallyreallyreallyreallyreallylongname",
            //     should_err: true,
            // },
            // This case should fail but due to the restriction of mock dependencies we cannot validate it correctly!
            // ValidatePathNameTestCase {
            //     name: "Standard address with backslash",
            //     path: r"\cosmos\1abcde\",
            //     should_err: true,
            // },
        ];

        for test in test_cases {
            let deps = mock_dependencies();
            let res = validate_path_name(&deps.api, test.path.to_string());
            assert_eq!(res.is_err(), test.should_err, "Test case: {}", test.name);
        }
    }

    struct ConvertComponentNameTestCase {
        name: &'static str,
        input: &'static str,
        expected: &'static str,
    }

    #[test]
    fn test_convert_component_name() {
        let test_cases: Vec<ConvertComponentNameTestCase> = vec![
            ConvertComponentNameTestCase {
                name: "Standard name with spaces",
                input: "Some Component Name",
                expected: "some_component_name",
            },
            ConvertComponentNameTestCase {
                name: "Name with hyphens",
                input: "Some-Component-Name",
                expected: "some-component-name",
            },
            ConvertComponentNameTestCase {
                name: "Name with uppercase letters",
                input: "SomeCOMPONENTName",
                expected: "somecomponentname",
            },
            ConvertComponentNameTestCase {
                name: "Name with numbers",
                input: "Component123",
                expected: "component123",
            },
            ConvertComponentNameTestCase {
                name: "Name with special characters",
                input: "Component!@#",
                expected: "component",
            },
            ConvertComponentNameTestCase {
                name: "Empty name",
                input: "",
                expected: "",
            },
            ConvertComponentNameTestCase {
                name: "Name with leading and trailing spaces",
                input: "  Some Component Name  ",
                expected: "some_component_name",
            },
            ConvertComponentNameTestCase {
                name: "Name with multiple spaces",
                input: "Some    Component    Name",
                expected: "some____component____name",
            },
        ];

        for test in test_cases {
            assert_eq!(
                convert_component_name(test.input),
                test.expected,
                "Test case: {}",
                test.name
            )
        }
    }

    struct ValidateUsernameTestCase {
        name: &'static str,
        username: &'static str,
        should_err: bool,
    }

    #[test]
    fn test_validate_username() {
        let test_cases: Vec<ValidateUsernameTestCase> = vec![
            ValidateUsernameTestCase {
                name: "Valid lowercase username",
                username: "validusername",
                should_err: false,
            },
            ValidateUsernameTestCase {
                name: "Valid numeric username",
                username: "123456",
                should_err: false,
            },
            ValidateUsernameTestCase {
                name: "Username with uppercase letters",
                username: "InvalidUsername",
                should_err: true,
            },
            ValidateUsernameTestCase {
                name: "Username with special characters",
                username: "user!@#",
                should_err: true,
            },
            ValidateUsernameTestCase {
                name: "Empty username",
                username: "",
                should_err: true,
            },
            ValidateUsernameTestCase {
                name: "Username with underscore",
                username: "valid_username",
                should_err: true,
            },
            ValidateUsernameTestCase {
                name: "Username with hyphen",
                username: "valid-username",
                should_err: true,
            },
            ValidateUsernameTestCase {
                name: "Username with period",
                username: "valid.username",
                should_err: true,
            },
            ValidateUsernameTestCase {
                name: "Username with leading numbers",
                username: "123validusername",
                should_err: false,
            },
            ValidateUsernameTestCase {
                name: "Username with only three characters",
                username: "usr",
                should_err: false,
            },
            ValidateUsernameTestCase {
                name: "Username with only one character",
                username: "a",
                should_err: true,
            },
            ValidateUsernameTestCase {
                name: "Username with whitespace",
                username: "valid username",
                should_err: true,
            },
            ValidateUsernameTestCase {
                name: "Username with leading whitespace",
                username: " validusername",
                should_err: true,
            },
            ValidateUsernameTestCase {
                name: "Username with trailing whitespace",
                username: "validusername ",
                should_err: true,
            },
            ValidateUsernameTestCase {
                name: "Username with mixed case letters",
                username: "ValidUserName",
                should_err: true,
            },
            ValidateUsernameTestCase {
                name: "Username with all uppercase letters",
                username: "VALIDUSERNAME",
                should_err: true,
            },
        ];

        for test in test_cases {
            assert_eq!(
                validate_username(test.username.to_string()).is_err(),
                test.should_err,
                "Test case: {}",
                test.name
            )
        }
    }
}
