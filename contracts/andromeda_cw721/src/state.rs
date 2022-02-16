use cosmwasm_std::Coin;
use cw721::Expiration;
use cw_storage_plus::{Index, IndexList, IndexedMap, MultiIndex};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Offer {
    pub amount: Coin,
    pub expiration: Expiration,
    pub purchaser: String,
}

pub struct OfferIndexes<'a> {
    /// (purchaser, token_id))
    pub purchaser: MultiIndex<'a, (String, Vec<u8>), Offer>,
}

impl<'a> IndexList<Offer> for OfferIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<Offer>> + '_> {
        let v: Vec<&dyn Index<Offer>> = vec![&self.purchaser];
        Box::new(v.into_iter())
    }
}

pub fn offers<'a>() -> IndexedMap<'a, &'a str, Offer, OfferIndexes<'a>> {
    let indexes = OfferIndexes {
        purchaser: MultiIndex::new(
            |e, k| (e.purchaser.clone(), k),
            "ownership",
            "offer_purchaser",
        ),
    };
    IndexedMap::new("ownership", indexes)
}

pub fn get_key(token_id: &str, purchaser: &str) -> Vec<u8> {
    return vec![token_id.as_bytes(), purchaser.as_bytes()].concat();
}
