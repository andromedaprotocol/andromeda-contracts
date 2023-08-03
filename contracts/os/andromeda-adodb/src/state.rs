use andromeda_std::os::adodb::{ADOVersion, ActionFee};
use cosmwasm_std::{Order, StdResult, Storage};
use cw_storage_plus::Map;

/// Stores a mapping from an ADO type/version to its code ID
pub const CODE_ID: Map<&str, u64> = Map::new("code_id");
/// Stores the latest version for a given ADO
pub const LATEST_VERSION: Map<&str, (String, u64)> = Map::new("latest_version");
/// Stores a mapping from code ID to ADO
pub const ADO_TYPE: Map<u64, String> = Map::new("ado_type");
/// Stores a mapping from ADO to its publisher
pub const PUBLISHER: Map<&str, String> = Map::new("publisher");
/// Stores a mapping from an (ADO,Action) to its action fees
pub const ACTION_FEES: Map<&(String, String), ActionFee> = Map::new("action_fees");

pub fn store_code_id(
    storage: &mut dyn Storage,
    ado_version: &ADOVersion,
    code_id: u64,
) -> StdResult<()> {
    ADO_TYPE
        .save(storage, code_id, &ado_version.clone().into_string())
        .unwrap();
    LATEST_VERSION
        .save(
            storage,
            &ado_version.get_type(),
            &(ado_version.clone().into_string(), code_id),
        )
        .unwrap();
    CODE_ID
        .save(storage, ado_version.as_str(), &code_id)
        .unwrap();

    // Check if there is any default ado set for this ado type. Defaults do not have versions appended to them.
    let default_ado = ADOVersion::from_type(ado_version.get_type());
    let default_code_id = read_code_id(storage, &default_ado);

    // There is no default, add one default for this
    if default_code_id.is_err() {
        CODE_ID
            .save(storage, default_ado.as_str(), &code_id)
            .unwrap();
    }
    Ok(())
}

pub fn read_code_id(storage: &dyn Storage, ado_version: &ADOVersion) -> StdResult<u64> {
    CODE_ID.load(storage, ado_version.as_str())
}

pub fn read_latest_code_id(storage: &dyn Storage, ado_type: String) -> StdResult<(String, u64)> {
    LATEST_VERSION.load(storage, &ado_type)
}

pub fn read_all_ado_types(storage: &dyn Storage) -> StdResult<Vec<String>> {
    let ado_types = CODE_ID
        .keys(storage, None, None, Order::Ascending)
        .flatten()
        .collect();
    Ok(ado_types)
}
