use crate::error::ContractError;
use crate::{ado_base::ownership::OwnershipMessage, ado_contract::ADOContract};
use cosmwasm_std::{attr, ensure, Addr, DepsMut, Env, MessageInfo, Response, Storage};
use cw_storage_plus::Item;
use cw_utils::Expiration;

const NEW_OWNER: Item<Addr> = Item::new("andr_new_owner");
const NEW_OWNER_EXPIRATION: Item<Expiration> = Item::new("andr_new_owner_expiration");

impl<'a> ADOContract<'a> {
    pub fn execute_ownership(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: OwnershipMessage,
    ) -> Result<Response, ContractError> {
        match msg {
            OwnershipMessage::UpdateOwner {
                new_owner,
                expiration,
            } => self.update_owner(deps, info, new_owner, expiration),
            OwnershipMessage::RevokeOwnershipOffer => self.revoke_ownership_offer(deps, info),
            OwnershipMessage::AcceptOwnership => self.accept_ownership(deps, env, info),
            OwnershipMessage::Disown => self.disown(deps, info),
            OwnershipMessage::UpdateOperators { new_operators } => {
                self.update_operators(deps, info, new_operators)
            }
        }
    }

    /// Updates the current contract owner. **Only executable by the current contract owner.**
    pub fn update_owner(
        &self,
        deps: DepsMut,
        info: MessageInfo,
        new_owner: Addr,
        expiration: Option<Expiration>,
    ) -> Result<Response, ContractError> {
        ensure!(
            self.is_contract_owner(deps.storage, info.sender.as_str())?,
            ContractError::Unauthorized {}
        );
        ensure!(
            !self.is_contract_owner(deps.storage, new_owner.as_str())?,
            ContractError::Unauthorized {}
        );
        let new_owner_addr = deps.api.addr_validate(new_owner.as_ref())?;
        NEW_OWNER.save(deps.storage, &new_owner_addr)?;

        if let Some(exp) = expiration {
            NEW_OWNER_EXPIRATION.save(deps.storage, &exp)?;
        } else {
            // In case an offer is already pending
            NEW_OWNER_EXPIRATION.remove(deps.storage);
        }

        Ok(Response::new().add_attributes(vec![
            attr("action", "update_owner"),
            attr("value", new_owner),
        ]))
    }

    /// Revokes the ownership offer. **Only executable by the current contract owner.**
    pub fn revoke_ownership_offer(
        &self,
        deps: DepsMut,
        info: MessageInfo,
    ) -> Result<Response, ContractError> {
        ensure!(
            self.is_contract_owner(deps.storage, info.sender.as_str())?,
            ContractError::Unauthorized {}
        );
        NEW_OWNER.remove(deps.storage);
        NEW_OWNER_EXPIRATION.remove(deps.storage);
        Ok(Response::new().add_attributes(vec![attr("action", "revoke_ownership_offer")]))
    }

    /// Accepts the ownership of the contract. **Only executable by the new contract owner.**
    pub fn accept_ownership(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
    ) -> Result<Response, ContractError> {
        let new_owner_addr = NEW_OWNER.load(deps.storage)?;
        ensure!(
            info.sender == new_owner_addr,
            ContractError::Unauthorized {}
        );
        let expiration = NEW_OWNER_EXPIRATION.may_load(deps.storage)?;
        if let Some(exp) = expiration {
            ensure!(!exp.is_expired(&env.block), ContractError::Unauthorized {});
        }

        self.owner.save(deps.storage, &new_owner_addr)?;
        NEW_OWNER.remove(deps.storage);
        NEW_OWNER_EXPIRATION.remove(deps.storage);
        Ok(Response::new().add_attributes(vec![
            attr("action", "accept_ownership"),
            attr("value", new_owner_addr.to_string()),
        ]))
    }

    /// Disowns the contract. **Only executable by the current contract owner.**
    pub fn disown(&self, deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
        ensure!(
            self.is_contract_owner(deps.storage, info.sender.as_str())?,
            ContractError::Unauthorized {}
        );
        self.owner.save(deps.storage, &Addr::unchecked("null"))?;
        Ok(Response::new().add_attributes(vec![attr("action", "disown")]))
    }

    /// Updates the current contract operators. **Only executable by the current contract owner.**
    pub fn update_operators(
        &self,
        deps: DepsMut,
        info: MessageInfo,
        operators: Vec<Addr>,
    ) -> Result<Response, ContractError> {
        ensure!(
            self.is_contract_owner(deps.storage, info.sender.as_str())?,
            ContractError::Unauthorized {}
        );
        self.operators.clear(deps.storage);
        for op in operators.iter() {
            self.operators.save(deps.storage, op.as_str(), &true)?;
        }

        Ok(Response::new().add_attributes(vec![attr("action", "update_operators")]))
    }

    /// Helper function to query if a given address is a operator.
    ///
    /// Returns a boolean value indicating if the given address is a operator.
    pub fn is_operator(&self, storage: &dyn Storage, addr: &str) -> bool {
        self.operators.has(storage, addr)
    }

    /// Helper function to query if a given address is the current contract owner.
    ///
    /// Returns a boolean value indicating if the given address is the contract owner.
    pub fn is_contract_owner(
        &self,
        storage: &dyn Storage,
        addr: &str,
    ) -> Result<bool, ContractError> {
        let owner = self.owner.load(storage)?;
        Ok(addr == owner)
    }

    /// Helper function to query if a given address is the current contract owner or operator.
    ///
    /// Returns a boolean value indicating if the given address is the contract owner or operator.
    pub fn is_owner_or_operator(
        &self,
        storage: &dyn Storage,
        addr: &str,
    ) -> Result<bool, ContractError> {
        Ok(self.is_contract_owner(storage, addr)? || self.is_operator(storage, addr))
    }
}

#[cfg(test)]
mod test {
    use cosmwasm_std::{
        testing::{mock_dependencies, mock_env, mock_info},
        Addr, DepsMut,
    };
    use cw_utils::Expiration;

    use crate::ado_contract::{
        ownership::{NEW_OWNER, NEW_OWNER_EXPIRATION},
        ADOContract,
    };

    fn init(deps: DepsMut, owner: impl Into<String>) {
        ADOContract::default()
            .owner
            .save(deps.storage, &Addr::unchecked(owner))
            .unwrap();
    }

    #[test]
    fn test_update_owner() {
        let mut deps = mock_dependencies();
        let contract = ADOContract::default();
        let new_owner = Addr::unchecked("new_owner");
        init(deps.as_mut(), "owner");

        let res = contract.update_owner(
            deps.as_mut(),
            mock_info("owner", &[]),
            new_owner.clone(),
            None,
        );
        assert!(res.is_ok());
        let saved_new_owner = NEW_OWNER.load(deps.as_ref().storage).unwrap();
        assert_eq!(saved_new_owner, new_owner);

        let res = contract.update_owner(
            deps.as_mut(),
            mock_info("owner", &[]),
            Addr::unchecked("owner"),
            None,
        );
        assert!(res.is_err());
        let res =
            contract.update_owner(deps.as_mut(), mock_info("new_owner", &[]), new_owner, None);
        assert!(res.is_err());
    }

    #[test]
    fn test_revoke_ownership_offer() {
        let mut deps = mock_dependencies();
        let contract = ADOContract::default();
        init(deps.as_mut(), "owner");

        let res = contract.revoke_ownership_offer(deps.as_mut(), mock_info("owner", &[]));
        assert!(res.is_ok());
        let saved_new_owner = NEW_OWNER.may_load(deps.as_ref().storage).unwrap();
        assert!(saved_new_owner.is_none());
    }

    #[test]
    fn test_accept_ownership() {
        let mut deps = mock_dependencies();
        let contract = ADOContract::default();
        let new_owner = Addr::unchecked("new_owner");
        init(deps.as_mut(), "owner");
        NEW_OWNER.save(deps.as_mut().storage, &new_owner).unwrap();

        let res = contract.accept_ownership(deps.as_mut(), mock_env(), mock_info("owner", &[]));
        assert!(res.is_err());
        let res = contract.accept_ownership(deps.as_mut(), mock_env(), mock_info("new_owner", &[]));
        assert!(res.is_ok());
        let saved_owner = contract.owner.load(deps.as_ref().storage).unwrap();
        assert_eq!(saved_owner, new_owner);
        let saved_new_owner = NEW_OWNER.may_load(deps.as_ref().storage).unwrap();
        assert!(saved_new_owner.is_none());
    }

    #[test]
    fn test_accept_ownership_expired() {
        let mut deps = mock_dependencies();
        let contract = ADOContract::default();
        let new_owner = Addr::unchecked("new_owner");
        init(deps.as_mut(), "owner");
        NEW_OWNER.save(deps.as_mut().storage, &new_owner).unwrap();
        NEW_OWNER_EXPIRATION
            .save(deps.as_mut().storage, &Expiration::AtHeight(1))
            .unwrap();

        let mut env = mock_env();
        env.block.height = 2;
        let res = contract.accept_ownership(deps.as_mut(), env, mock_info("new_owner", &[]));
        assert!(res.is_err());
        let saved_owner = contract.owner.load(deps.as_ref().storage).unwrap();
        assert_eq!(saved_owner, Addr::unchecked("owner"));
    }

    #[test]
    fn test_disown() {
        let mut deps = mock_dependencies();
        let contract = ADOContract::default();
        init(deps.as_mut(), "owner");

        let res = contract.disown(deps.as_mut(), mock_info("owner", &[]));
        assert!(res.is_ok());
        let saved_owner = contract.owner.load(deps.as_ref().storage).unwrap();
        assert_eq!(saved_owner, Addr::unchecked("null"));
    }

    #[test]
    fn test_update_operators() {
        let mut deps = mock_dependencies();
        let contract = ADOContract::default();
        let new_operators = vec![Addr::unchecked("new_operator")];
        init(deps.as_mut(), "owner");

        let res = contract.update_operators(
            deps.as_mut(),
            mock_info("owner", &[]),
            new_operators.clone(),
        );
        assert!(res.is_ok());
        for op in new_operators {
            let is_operator = contract
                .operators
                .load(deps.as_ref().storage, op.as_str())
                .unwrap();
            assert!(is_operator);
        }
    }
}
