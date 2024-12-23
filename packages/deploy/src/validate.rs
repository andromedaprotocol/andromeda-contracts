use std::env;

use crate::{
    chains::get_chain,
    contracts::get_contract,
    contracts::{all_contracts, DeployableContract},
};
use andromeda_adodb::ADODBContract;
use andromeda_kernel::KernelContract;
use andromeda_std::os::{
    adodb::QueryMsgFns as ADODBQueryMsgFns, kernel::QueryMsgFns as KernelQueryMsgFns,
};
use cosmwasm_std::Addr;
use cw_orch::prelude::*;
use cw_orch_daemon::{DaemonBase, DaemonBuilder, Wallet};

/// Creates and returns an ADODB contract instance connected to the specified chain.
///
/// This function:
/// 1. Retrieves the chain configuration using the DEPLOYMENT_CHAIN environment variable
/// 2. Creates a daemon connection to the chain
/// 3. Sets up a kernel contract instance to fetch the ADODB address
/// 4. Creates and returns an ADODB contract instance
///
/// # Panics
///
/// - If DEPLOYMENT_CHAIN environment variable is not set
/// - If ADODB address is not set in the kernel
/// - If chain configuration is invalid
fn get_adodb_contract() -> ADODBContract<DaemonBase<Wallet>> {
    let chain_id = dotenv::var("DEPLOYMENT_CHAIN").unwrap();
    let chain = get_chain(chain_id);
    let daemon = DaemonBuilder::new(chain).build().unwrap();

    let kernel = KernelContract::new(daemon.clone());
    let kernel_address =
        dotenv::var("DEPLOYMENT_KERNEL_ADDRESS").expect("DEPLOYMENT_KERNEL_ADDRESS is not set");
    kernel.set_address(&Addr::unchecked(kernel_address.clone()));
    let adodb_addr = kernel
        .key_address("adodb")
        .unwrap_or_else(|_| panic!("ADODB address not set for provided kernel"));
    let adodb = ADODBContract::new(daemon.clone());
    adodb.set_address(&adodb_addr);
    adodb
}

/// Validates the chain configuration specified in environment variables.
///
/// This function ensures that:
/// 1. The DEPLOYMENT_CHAIN environment variable is set
/// 2. The specified chain ID has a valid chain configuration
///
/// # Panics
///
/// - If DEPLOYMENT_CHAIN environment variable is not set
/// - If the specified chain ID doesn't have a valid configuration
fn validate_chain_configs() {
    // Validate Chain ID
    let chain_id = dotenv::var("DEPLOYMENT_CHAIN").expect("DEPLOYMENT_CHAIN is not set");
    // Validates that the provided chain ID has a chain config
    get_chain(chain_id);
}

/// Validates the kernel address configuration based on deployment settings.
///
/// This function checks that either:
/// 1. A kernel address is provided via DEPLOYMENT_KERNEL_ADDRESS, or
/// 2. The DEPLOY_OS flag is set to true
///
/// # Panics
///
/// - If neither DEPLOYMENT_KERNEL_ADDRESS is set nor DEPLOY_OS is true
fn validate_kernel_address() {
    // Validate Kernel Address
    let kernel_address = dotenv::var("DEPLOYMENT_KERNEL_ADDRESS").ok();
    let deploy_os = dotenv::var("DEPLOY_OS")
        .unwrap_or("false".to_string())
        .to_lowercase()
        == "true";

    // Either deploy OS must be true or kernel address must be set
    if !deploy_os && kernel_address.is_none() {
        panic!("DEPLOYMENT_KERNEL_ADDRESS is not set");
    }
}

/// Filters out invalid contracts from the deployment list and updates the environment.
///
/// This function:
/// 1. Reads the DEPLOY_CONTRACTS environment variable
/// 2. Identifies and logs any invalid contract names
/// 3. Updates DEPLOY_CONTRACTS to only include valid contracts
///
/// If DEPLOY_CONTRACTS is empty, the function returns early without modification.
///
/// # Panics
///
/// - If all specified contracts are invalid (resulting in an empty list)
fn filter_invalid_contracts() {
    // Validate Deployment Contracts
    let deployment_contracts = dotenv::var("DEPLOY_CONTRACTS").unwrap_or_default();
    if deployment_contracts.is_empty() {
        return;
    }

    let deployment_contracts = deployment_contracts
        .split(',')
        .map(|c| c.trim().to_string());

    // Determine invalid contracts for logging purposes
    let invalid_contracts: Vec<String> = deployment_contracts
        .clone()
        .filter(|contract_name| get_contract(contract_name.clone()).is_none())
        .collect();

    if !invalid_contracts.is_empty() {
        log::warn!("Contracts not found: {}", invalid_contracts.join(", "));
    }

    // Filter out invalid contracts
    let valid_contracts: Vec<String> = deployment_contracts
        .filter(|contract_name| get_contract(contract_name.clone()).is_some())
        .collect();

    if valid_contracts.is_empty() {
        panic!("Provided contracts empty after filtering invalid contracts");
    }

    log::info!("Setting deploy contracts: {}", valid_contracts.join(","));
    env::set_var("DEPLOY_CONTRACTS", valid_contracts.join(","));
}

/// Filters out contracts that are already deployed on the target chain.
///
/// This function:
/// 1. Checks for deployed contracts using the kernel and ADODB addresses
/// 2. Compares existing contract versions with requested versions
/// 3. Updates DEPLOY_CONTRACTS env var with only non-deployed contracts
///
/// # Panics
///
/// - If ADODB address is not set in the kernel
/// - If there are no contracts left to deploy after filtering
fn filter_deployed_contracts() {
    let kernel_address = dotenv::var("DEPLOYMENT_KERNEL_ADDRESS").ok();
    if kernel_address.is_none() {
        log::debug!("No kernel address provided, skipping deployed contracts filter");
        return;
    }

    let kernel_address = kernel_address.unwrap();
    log::debug!(
        "Filtering deployed contracts for kernel: {}",
        kernel_address
    );

    let deployment_contracts = dotenv::var("DEPLOY_CONTRACTS").unwrap_or_default();
    log::debug!("Raw deployment contracts: {}", deployment_contracts);

    let valid_contracts: Vec<String> = deployment_contracts
        .split(',')
        .map(|c| c.trim().to_string())
        .filter(|contract_name| get_contract(contract_name.clone()).is_some())
        .collect();

    log::debug!("Valid contracts after filtering: {:?}", valid_contracts);

    let contracts_to_validate: Vec<DeployableContract> = if valid_contracts.is_empty() {
        log::info!("No specific contracts specified, validating all available contracts");
        all_contracts().into_iter().collect()
    } else {
        log::info!(
            "Filtering contracts to specified list: {:?}",
            valid_contracts
        );
        all_contracts()
            .into_iter()
            .filter(|(name, _, _)| valid_contracts.contains(name))
            .collect()
    };
    if contracts_to_validate.is_empty() {
        panic!("UNEXPECTED: No contracts to validate");
    }

    let adodb = get_adodb_contract();
    log::debug!(
        "Checking deployed versions on ADODB at: {}",
        adodb.address().unwrap().to_string()
    );
    // Filter out contracts that are already deployed based on their versions
    let deployable_contracts: Vec<String> = contracts_to_validate
        .into_iter()
        .filter(|(name, version, _)| {
            let versions = adodb.ado_versions(name, None, None).unwrap_or_default();
            let should_deploy = !versions.contains(&format!("{}@{}", name, version));
            if !should_deploy {
                log::debug!("Skipping {}: version {} already deployed", name, version);
            }
            should_deploy
        })
        .map(|(name, _, _)| name)
        .collect();

    log::info!("Contracts requiring deployment: {:?}", deployable_contracts);
    let deploy_os = dotenv::var("DEPLOY_OS")
        .unwrap_or("false".to_string())
        .to_lowercase()
        == "true";

    if deployable_contracts.is_empty() && deploy_os {
        // If we don't have any contracts to deploy but we want to deploy the OS we still need to build the OS contracts
        log::warn!("No contracts to deploy - all specified contracts are already deployed. Continuing with OS deployment...");
        // This will skip the build step for all non-OS contracts
        env::set_var("DEPLOYMENT_SKIP_BUILD", "true");
    } else if deployable_contracts.is_empty() {
        // If we don't have any contracts to deploy and we don't want to deploy the OS we should panic as there is nothing to do
        panic!("No contracts to deploy - all specified contracts are already deployed");
    }

    // Check if we are deploying all contracts
    let is_deploying_all_contracts = all_contracts()
        .into_iter()
        .map(|(name, _, _)| name.clone())
        .collect::<Vec<String>>()
        == deployable_contracts;

    if is_deploying_all_contracts {
        // If we are deploying all contracts we need to remove the DEPLOY_CONTRACTS env var
        env::remove_var("DEPLOY_CONTRACTS");
    } else {
        // If we are not deploying all contracts we need to set the DEPLOY_CONTRACTS env var
        env::set_var("DEPLOY_CONTRACTS", deployable_contracts.join(","));
    };
}

/// Validates and filters the contracts that will be deployed.
///
/// This function performs two validation steps:
/// 1. Filters out invalid contracts that don't exist in the system using `filter_invalid_contracts`
/// 2. Filters out contracts that are already deployed on the target chain using `filter_deployed_contracts`
///
/// The function modifies the `DEPLOY_CONTRACTS` environment variable to contain only valid,
/// non-deployed contracts.
///
/// # Panics
///
/// - If all specified contracts are invalid
/// - If all contracts are already deployed and OS deployment is not requested
fn validate_deployment_contracts() {
    filter_invalid_contracts();
    filter_deployed_contracts();
}

pub fn run() {
    validate_chain_configs();
    validate_kernel_address();
    validate_deployment_contracts();
}
