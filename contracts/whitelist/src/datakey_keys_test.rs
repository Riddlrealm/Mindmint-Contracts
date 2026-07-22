#![cfg(test)]
use soroban_sdk::{symbol_short, Address, Env, IntoVal, vec};

use crate::DataKey;

/// Verifies that every `DataKey` variant serializes to a distinct storage key,
/// preventing the cross-proxy key collisions described in Issue #25.
#[test]
fn test_datakey_variant_keys_are_unique() {
    let env = Env::default();
    let addr = Address::generate(&env);

    let variants = vec![
        DataKey::Admin.into_val(&env),
        DataKey::Entry(addr.clone()).into_val(&env),
        DataKey::MerkleRoot.into_val(&env),
        DataKey::Snapshot.into_val(&env),
        DataKey::TierPermissions(1u32).into_val(&env),
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

    let raw = symbol_short!("ADMIN").into_val(&env);
    assert_ne!(variants[0], raw, "DataKey::Admin collides with raw 'ADMIN'");
}
