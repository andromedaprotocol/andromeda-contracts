use andromeda_address_list::AddressListContract;
use andromeda_adodb::ADODBContract;
use andromeda_app_contract::AppContract;
use andromeda_auction::AuctionContract;
use andromeda_boolean::BooleanContract;
use andromeda_conditional_splitter::ConditionalSplitterContract;
use andromeda_counter::CounterContract;
use andromeda_crowdfund::CrowdfundContract;
use andromeda_curve::CurveContract;
use andromeda_cw20::CW20Contract;
use andromeda_cw20_exchange::Cw20ExchangeContract;
use andromeda_cw20_staking::CW20StakingContract;
use andromeda_cw721::CW721Contract;
use andromeda_distance::DistanceContract;
use andromeda_economics::EconomicsContract;
use andromeda_ibc_registry::IBCRegistryContract;
use andromeda_kernel::KernelContract;
use andromeda_lockdrop::LockdropContract;
use andromeda_marketplace::MarketplaceContract;
use andromeda_merkle_airdrop::MerkleAirdropContract;
use andromeda_primitive::PrimitiveContract;
use andromeda_rate_limiting_withdrawals::RateLimitingWithdrawalsContract;
use andromeda_rates::RatesContract;
use andromeda_set_amount_splitter::SetAmountSplitterContract;
use andromeda_shunting::ShuntingContract;
use andromeda_splitter::SplitterContract;
use andromeda_std::deploy::ADOMetadata;
use andromeda_string_storage::StringStorageContract;
use andromeda_timelock::TimelockContract;
use andromeda_validator_staking::ValidatorStakingContract;
use andromeda_vesting::VestingContract;
use andromeda_vfs::VFSContract;
use andromeda_weighted_distribution_splitter::WeightedDistributionSplitterContract;

use cw_orch::prelude::*;
use cw_orch_daemon::{DaemonBase, Wallet};

type UploadFn = Box<dyn FnOnce(&DaemonBase<Wallet>) -> Result<u64, CwOrchError>>;
pub type DeployableContract = (String, String, UploadFn);

/// Macro to create a tuple of (name, version, uploadFn) for a given contract.
macro_rules! deployable {
    ($contract_struct:ident) => {
        (
            $contract_struct::<DaemonBase<Wallet>>::name(),
            $contract_struct::<DaemonBase<Wallet>>::version(),
            Box::new(|chain: &DaemonBase<Wallet>| {
                let contract = $contract_struct::<DaemonBase<Wallet>>::new(chain.clone());
                contract.upload()?;
                Ok(contract.code_id().unwrap())
            }),
        )
    };
}

pub fn all_contracts() -> Vec<DeployableContract> {
    vec![
        deployable!(SplitterContract),
        deployable!(ValidatorStakingContract),
        deployable!(VestingContract),
        deployable!(TimelockContract),
        deployable!(CounterContract),
        deployable!(PrimitiveContract),
        deployable!(CW20Contract),
        deployable!(CW20StakingContract),
        deployable!(CW721Contract),
        deployable!(AppContract),
        deployable!(BooleanContract),
        deployable!(StringStorageContract),
        deployable!(ConditionalSplitterContract),
        deployable!(RateLimitingWithdrawalsContract),
        deployable!(SetAmountSplitterContract),
        deployable!(WeightedDistributionSplitterContract),
        deployable!(Cw20ExchangeContract),
        deployable!(LockdropContract),
        deployable!(MerkleAirdropContract),
        deployable!(AddressListContract),
        deployable!(CurveContract),
        // Undeployable for now
        // deployable!(DateTimeContract),
        deployable!(RatesContract),
        deployable!(ShuntingContract),
        deployable!(AuctionContract),
        deployable!(CrowdfundContract),
        deployable!(MarketplaceContract),
        deployable!(DistanceContract),
    ]
}

pub fn os_contracts() -> Vec<DeployableContract> {
    vec![
        deployable!(ADODBContract),
        deployable!(KernelContract),
        deployable!(VFSContract),
        deployable!(EconomicsContract),
        deployable!(IBCRegistryContract),
    ]
}

pub fn get_contract(contract_name: String) -> Option<DeployableContract> {
    all_contracts()
        .into_iter()
        .chain(os_contracts())
        .find(|(name, _, _)| *name == contract_name)
}
