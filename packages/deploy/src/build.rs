use std::process::Command;

fn get_contracts_to_build() -> Vec<String> {
    let should_deploy_os = dotenv::var("DEPLOY_OS")
        .unwrap_or("false".to_string())
        .to_lowercase()
        == "true";
    let contracts_to_build = dotenv::var("DEPLOY_CONTRACTS").unwrap_or_default();
    if contracts_to_build.is_empty() && !should_deploy_os {
        return vec![];
    }
    let mut contracts: Vec<String> = contracts_to_build
        .split(",")
        .filter(|c| !c.is_empty())
        .map(|c| format!("andromeda-{}", c.to_string().trim()).to_string())
        .filter(|c| !c.is_empty())
        .collect();

    if should_deploy_os {
        log::debug!("Adding OS contract to build list");
        contracts.push("os".to_string());
    }
    contracts
}

pub fn build_schemas(contracts: Vec<String>) {
    let output = Command::new("bash")
        .arg("./scripts/build_schema.sh")
        .args(contracts)
        .output()
        .expect("Failed to execute build schema script");

    if !output.status.success() {
        eprintln!("Schema build failed with error:");
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("Build script failed with status: {}", output.status);
    }
}

pub fn build_contracts(contracts: Vec<String>) {
    let output = Command::new("bash")
        .arg("./scripts/build.sh")
        .args(contracts)
        .output()
        .expect("Failed to execute build script");

    if !output.status.success() {
        eprintln!("Contract build failed with error:");
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("Build script failed with status: {}", output.status);
    }
}

pub fn build_all_contracts() {
    let status = Command::new("make")
        .arg("build")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("Failed to execute make command");

    if !status.success() {
        panic!("Build command failed with status: {}", status);
    }
}

/// Builds smart contracts based on environment variables configuration.
///
/// # Environment Variables
/// - `DEPLOYMENT_SKIP_BUILD`: If "true", skips the build process entirely
/// - `DEPLOY_CONTRACTS`: Comma-separated list of contracts to build
/// - `DEPLOY_OS`: If "true", adds the OS contract to the build list
///
/// # Behavior
/// - If no contracts are specified, builds all contracts
/// - If specific contracts are listed, builds only those contracts
/// - Automatically prefixes contract names with "andromeda-" if needed
pub fn run() {
    let should_skip_build =
        dotenv::var("DEPLOYMENT_SKIP_BUILD").unwrap_or("false".to_string()) == "true";
    let deploy_os = dotenv::var("DEPLOY_OS")
        .unwrap_or("false".to_string())
        .to_lowercase()
        == "true";
    if should_skip_build && !deploy_os {
        log::info!("Build process skipped due to DEPLOYMENT_SKIP_BUILD=true and DEPLOY_OS=false");
        return;
    }

    let contracts_to_build = dotenv::var("DEPLOY_CONTRACTS").unwrap_or_default();
    log::debug!(
        "Contracts specified in DEPLOY_CONTRACTS: {}",
        contracts_to_build
    );

    let named_contracts: Vec<String> = get_contracts_to_build();
    log::info!("Building schemas...");
    build_schemas(named_contracts.clone());
    log::info!("Building contracts...");
    if named_contracts.is_empty() && !should_skip_build {
        log::info!("No specific contracts specified, building all contracts");
        build_all_contracts();
    } else {
        log::info!("Building specific contracts: {:?}", named_contracts);
        build_contracts(named_contracts);
    }
}
