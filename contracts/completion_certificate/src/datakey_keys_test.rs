#![cfg(test)]
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, vec, xdr::ToXdr, Address, Env, String};

use crate::DataKey;

/// Verifies that every `DataKey` variant serializes to a distinct storage key,
/// preventing the cross-proxy key collisions described in Issue #25.
#[test]
fn test_datakey_variant_keys_are_unique() {
    let env = Env::default();
    let addr = Address::generate(&env);
    let pid = String::from_str(&env, "PUZZLE-1");

    let variants = vec![
        &env,
        DataKey::Admin.to_xdr(&env),
        DataKey::TokenCount.to_xdr(&env),
        DataKey::Paused.to_xdr(&env),
        DataKey::Cert(1u64).to_xdr(&env),
        DataKey::OwnerCerts(addr.clone()).to_xdr(&env),
        DataKey::PuzzleMinted(pid.clone(), addr.clone()).to_xdr(&env),
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

    let raw = symbol_short!("ADMIN").to_xdr(&env);
    assert_ne!(
        variants.get(0).unwrap(),
        raw,
        "DataKey::Admin collides with raw 'ADMIN'"
    );
}
