#![cfg(test)]
use soroban_sdk::{symbol_short, Address, Env, IntoVal, Symbol, vec};

use crate::DataKey;

/// Verifies that every `DataKey` variant serializes to a distinct storage key,
/// preventing the cross-proxy key collisions described in Issue #25.
#[test]
fn test_datakey_variant_keys_are_unique() {
    let env = Env::default();
    let addr = Address::generate(&env);
    let sym = Symbol::new(&env, "solve");

    let variants = vec![
        DataKey::Config.into_val(&env),
        DataKey::Oracles.into_val(&env),
        DataKey::ProofCounter.into_val(&env),
        DataKey::NextProofId.into_val(&env),
        DataKey::Proof(1u64).into_val(&env),
        DataKey::ActivityCount(addr.clone(), 1u32).into_val(&env),
        DataKey::ActivityScore(addr.clone()).into_val(&env),
    ];

    for i in 0..variants.len() {
        for j in (i + 1)..variants.len() {
            assert_ne!(
                variants[i], variants[j],
                "storage-key collision between DataKey variants {} and {}",
                i, j
            );
        }
    }

    let raw = symbol_short!("OR_CFG").into_val(&env);
    assert_ne!(variants[0], raw, "DataKey::Config collides with raw 'OR_CFG'");
}
