#[macro_export]
macro_rules! contract_interface {
    ($contract_name:ident, $module_path:ident, $package_path:ident, $contract_id:expr, $wasm_path:expr) => {
        #[interface($package_path::InstantiateMsg, $package_path::ExecuteMsg, $package_path::QueryMsg, MigrateMsg, id = $contract_id)]
        pub struct $contract_name;

        impl<Chain> Uploadable for $contract_name<Chain> {
            fn wrapper() -> Box<dyn MockContract<Empty>> {
                Box::new(
                    ContractWrapper::new_with_empty(
                        $module_path::contract::execute,
                        $module_path::contract::instantiate,
                        $module_path::contract::query,
                    )
                    .with_migrate($module_path::contract::migrate),
                )
            }

            fn wasm(_chain: &ChainInfoOwned) -> WasmPath {
                artifacts_dir_from_workspace!()
                    .find_wasm_path(&format!("andromeda_{}.wasm", $wasm_path))
                    .unwrap()
            }
        }
    };
}
