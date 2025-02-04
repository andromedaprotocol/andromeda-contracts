use andromeda_std::{common::context::ExecuteContext, error::ContractError};
use bech32::{ToBase32, Variant};
#[cfg(not(feature = "library"))]
use cosmwasm_std::ensure;
use ripemd::Ripemd160;
use secp256k1::PublicKey;
use sha2::{digest::Update, Digest, Sha256};

#[allow(dead_code)]
pub fn verify_signature(
    ctx: ExecuteContext,
    msg: String,
    signature: &[u8],
    public_key: &[u8],
    signer_addr: String,
    address_prefix: String,
) -> Result<bool, ContractError> {
    let address = derive_address(&address_prefix, public_key).unwrap();
    ensure!(
        address == signer_addr,
        ContractError::InvalidSigner {
            signer: signer_addr
        }
    );

    let message_hash: [u8; 32] = Sha256::new().chain(&msg).finalize().into();

    match ctx
        .deps
        .api
        .secp256k1_verify(&message_hash, signature, public_key)
    {
        Ok(valid) => Ok(valid),
        Err(_) => Ok(false),
    }
}

#[allow(dead_code)]
pub fn derive_address(prefix: &str, public_key_bytes: &[u8]) -> Result<String, ContractError> {
    let pub_key_compressed = &PublicKey::from_slice(public_key_bytes)
        .map_err(|_| ContractError::InvalidPublickey {})?
        .serialize();

    // Hash with SHA-256
    let sha256_hash = Sha256::digest(pub_key_compressed);

    // Hash with RIPEMD-160
    let ripemd160_hash = Ripemd160::digest(sha256_hash);

    // Encode with bech32
    bech32::encode(prefix, ripemd160_hash.to_base32(), Variant::Bech32)
        .map_err(|_| ContractError::InvalidAddress {})
}

#[cfg(test)]
mod tests {
    use super::{derive_address, verify_signature};
    use andromeda_std::common::context::ExecuteContext;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use k256::{ecdsa::SigningKey, elliptic_curve::rand_core::OsRng};
    use sha2::{digest::Update, Digest, Sha256};

    #[test]
    fn test_verify_signature() {
        let msg: String = "Hello World!".to_string();

        // Signing
        let message_digest = Sha256::new().chain(msg.clone());

        let secret_key = SigningKey::random(&mut OsRng);
        let signature = secret_key
            .sign_digest_recoverable(message_digest)
            .unwrap()
            .0;

        let public_key = secret_key.verifying_key();
        let binding = public_key.to_encoded_point(false);
        let public_key_bytes = binding.as_bytes();

        // verifying
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("owner", &[]);
        let address = derive_address("neutron", public_key_bytes).unwrap();

        let ctx = ExecuteContext::new(deps.as_mut(), info, env);
        assert!(verify_signature(
            ctx,
            msg.clone(),
            &signature.to_bytes(),
            public_key_bytes,
            address.clone(),
            "neutron".to_string()
        )
        .unwrap());
    }
}
