#![cfg(test)]
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, vec, xdr::ToXdr, Address, Env};

use crate::DataKey;

/// Verifies that every `DataKey` variant serializes to a distinct storage key,
/// preventing the cross-proxy key collisions described in Issue #25.
#[test]
fn test_datakey_variant_keys_are_unique() {
    let env = Env::default();
    let addr = Address::generate(&env);

    let variants = vec![
        &env,
        DataKey::Config.to_xdr(&env),
        DataKey::Oracles.to_xdr(&env),
        DataKey::ProofCounter.to_xdr(&env),
        DataKey::NextProofId.to_xdr(&env),
        DataKey::Proof(1u64).to_xdr(&env),
        DataKey::ActivityCount(addr.clone(), 1u32).to_xdr(&env),
        DataKey::ActivityScore(addr.clone()).to_xdr(&env),
    ];

    for i in 0..variants.len() {
        for j in (i + 1)..variants.len() {
            assert_ne!(
                variants.get(i).unwrap(),
                variants.get(j).unwrap(),
                "storage-key collision between DataKey variants {} and {}",
                i,
                j
            );
        }
    }

    let raw = symbol_short!("OR_CFG").to_xdr(&env);
    assert_ne!(
        variants.get(0).unwrap(),
        raw,
        "DataKey::Config collides with raw 'OR_CFG'"
    );
}
