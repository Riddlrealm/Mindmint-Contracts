#![cfg(test)]
use soroban_sdk::{symbol_short, vec, xdr::ToXdr, Env};

use crate::storage::DataKey;

/// Verifies that every `DataKey` variant serializes to a distinct storage key.
/// `liquidity_pool`'s `DataKey` is a `#[contracttype]` enum, so distinct variants
/// serialize to distinct ledger keys — the fix for the cross-proxy key collisions
/// described in Issue #25.
#[test]
fn test_datakey_variant_keys_are_unique() {
    let env = Env::default();

    let variants = vec![
        &env,
        DataKey::Initialized.to_xdr(&env),
        DataKey::Admin.to_xdr(&env),
        DataKey::TokenA.to_xdr(&env),
        DataKey::TokenB.to_xdr(&env),
        DataKey::ReserveA.to_xdr(&env),
        DataKey::ReserveB.to_xdr(&env),
        DataKey::TotalSupply.to_xdr(&env),
        DataKey::FeeBps.to_xdr(&env),
        DataKey::FeeRecipient.to_xdr(&env),
        DataKey::FeesA.to_xdr(&env),
        DataKey::FeesB.to_xdr(&env),
        DataKey::PriceOracleTimestamp.to_xdr(&env),
        DataKey::CumulativePrice.to_xdr(&env),
        DataKey::Balance.to_xdr(&env),
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

    let raw = symbol_short!("admin").to_xdr(&env);
    assert_ne!(
        variants.get(1).unwrap(),
        raw,
        "DataKey::Admin collides with raw 'admin'"
    );
}
