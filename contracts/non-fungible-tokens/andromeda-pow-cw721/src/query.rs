use andromeda_non_fungible_tokens::pow_cw721::{GetLinkedCw721AddressResponse, GetPowNFTResponse};
use andromeda_std::error::ContractError;
use cosmwasm_std::Deps;

use crate::state::{LINKED_CW721_ADDRESS, POW_NFT};

pub fn query_pow_nft(deps: Deps, token_id: String) -> Result<GetPowNFTResponse, ContractError> {
    let pow_nft = POW_NFT
        .load(deps.storage, token_id)
        .map_err(|_| ContractError::NFTNotFound {})?;

    Ok(GetPowNFTResponse {
        nft_response: pow_nft,
    })
}

pub fn query_linked_cw721_address(
    deps: Deps,
) -> Result<GetLinkedCw721AddressResponse, ContractError> {
    let linked_cw721_address = LINKED_CW721_ADDRESS.load(deps.storage)?;

    Ok(GetLinkedCw721AddressResponse {
        linked_cw721_address,
    })
}
