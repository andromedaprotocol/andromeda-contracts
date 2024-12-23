pub mod devnets;
pub mod mainnets;
pub mod testnets;

use cw_orch::prelude::ChainInfo;
use devnets::DEVNET_CHAINS;
use mainnets::MAINNET_CHAINS;
use testnets::TESTNET_CHAINS;

pub fn get_chain(chain: String) -> ChainInfo {
    let all_chains: Vec<ChainInfo> = [MAINNET_CHAINS, TESTNET_CHAINS, DEVNET_CHAINS].concat();
    let unique_chain_names: std::collections::HashSet<&str> = all_chains
        .iter()
        .map(|c| c.network_info.chain_name)
        .collect();
    if unique_chain_names.len() != all_chains.len() {
        panic!("Duplicate chain names found in ChainInfo");
    }

    all_chains
        .iter()
        .find(|c| c.chain_id == chain || c.network_info.chain_name == chain)
        .unwrap_or_else(|| panic!("Chain {} not found", chain))
        .clone()
}
