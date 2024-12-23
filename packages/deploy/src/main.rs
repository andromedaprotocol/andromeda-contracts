use andromeda_deploy::build;
use andromeda_deploy::report::DeploymentReport;
use andromeda_deploy::slack::SlackNotification;
use andromeda_deploy::validate;
use std::env;

use andromeda_deploy::adodb;
use andromeda_deploy::os;
use dotenv::dotenv;

fn main() {
    env_logger::init();
    dotenv().ok();

    validate::run();

    build::run();

    let chain = env::var("DEPLOYMENT_CHAIN").expect("DEPLOYMENT_CHAIN must be set");
    let mut kernel_address = env::var("DEPLOYMENT_KERNEL_ADDRESS").ok();

    // Send start notification
    SlackNotification::DeploymentStarted(chain.clone(), kernel_address.clone())
        .send()
        .unwrap();

    let should_deploy_os = env::var("DEPLOY_OS").unwrap_or_default().to_lowercase() == "true";
    if should_deploy_os {
        let deploy_os_res = os::deploy(chain.clone(), kernel_address.clone());

        if let Err(e) = deploy_os_res {
            println!("Error deploying OS: {}", e);
            SlackNotification::DeploymentFailed(chain.clone(), kernel_address.clone(), e)
                .send()
                .unwrap();
            std::process::exit(1);
        }

        kernel_address = Some(deploy_os_res.unwrap());

        // Send completion notification
        SlackNotification::DeploymentCompleted(chain.clone(), kernel_address.clone())
            .send()
            .unwrap();
    }

    let kernel_address = kernel_address.expect("Kernel address must be set or OS deployed");
    let contracts_to_deploy = env::var("DEPLOY_CONTRACTS")
        .ok()
        .unwrap_or_default()
        .split(',')
        .map(|s| {
            s.to_string()
                .trim()
                .to_lowercase()
                .replace("andromeda_", "")
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<String>>();

    let adodb_res = adodb::deploy(
        chain.clone(),
        kernel_address.clone(),
        Some(contracts_to_deploy),
    );
    if let Err(e) = adodb_res {
        println!("Error deploying ADODB: {}", e);
        SlackNotification::ADODeploymentFailed(chain.clone(), e)
            .send()
            .unwrap();
        std::process::exit(1);
    }

    let deployed_contracts = adodb_res.unwrap();
    DeploymentReport {
        chain_id: chain.clone(),
        contracts: deployed_contracts,
        kernel_address,
    }
    .write_to_json()
    .unwrap();
}
