use crate::slack::SlackNotification;
use crate::{chains::get_chain, contracts::all_contracts, error::DeployError};
use adodb::ExecuteMsgFns;
use andromeda_adodb::ADODBContract;
use andromeda_kernel::KernelContract;
use andromeda_std::os::*;
use cw_orch::prelude::*;
use cw_orch_daemon::DaemonBuilder;
use kernel::QueryMsgFns;

pub fn deploy(
    chain: String,
    kernel_address: String,
    contracts: Option<Vec<String>>,
) -> Result<(), DeployError> {
    let chain = get_chain(chain);
    let daemon = DaemonBuilder::new(chain.clone()).build().unwrap();
    let kernel = KernelContract::new(daemon.clone());
    kernel.set_address(&Addr::unchecked(kernel_address.clone()));

    let adodb = ADODBContract::new(daemon.clone());
    let adodb_addr = kernel.key_address("adodb")?;
    adodb.set_address(&adodb_addr);

    let all_contracts = all_contracts();

    let contracts_to_deploy = contracts.unwrap_or_default();
    let invalid_contracts = contracts_to_deploy
        .iter()
        .filter(|name| !all_contracts.iter().any(|(n, _, _)| &n == name))
        .cloned()
        .collect::<Vec<String>>();
    if !invalid_contracts.is_empty() {
        SlackNotification::ADOWarning(chain.chain_id.to_string(), invalid_contracts.clone())
            .send()
            .unwrap();
    }

    let valid_contracts = contracts_to_deploy
        .iter()
        .filter(|name| all_contracts.iter().any(|(n, _, _)| &n == name))
        .cloned()
        .collect::<Vec<String>>();

    SlackNotification::ADODeploymentStarted(chain.chain_id.to_string(), valid_contracts.clone())
        .send()
        .unwrap();

    let mut deployed_contracts: Vec<(String, String, u64)> = vec![];
    for (name, version, upload) in all_contracts {
        if !contracts_to_deploy.is_empty() && !contracts_to_deploy.contains(&name) {
            continue;
        }

        println!("Deploying {} {}", name, version);
        let code_id = upload(&daemon)?;
        adodb.publish(name.clone(), code_id, version.clone(), None, None)?;
        deployed_contracts.push((name, version, code_id));
    }

    SlackNotification::ADODeploymentCompleted(chain.chain_id.to_string(), valid_contracts.clone())
        .send()
        .unwrap();

    Ok(())
}
