#![cfg(test)]
use soroban_sdk::{symbol_short, Env, IntoVal, vec};

use crate::storage::DataKey;

/// Verifies that every `DataKey` variant serializes to a distinct storage key.
/// `liquidity_pool`'s `DataKey` is a `#[contracttype]` enum, so distinct variants
/// serialize to distinct ledger keys — the fix for the cross-proxy key collisions
/// described in Issue #25.
#[test]
fn test_datakey_variant_keys_are_unique() {
    let env = Env::default();

    let variants = vec![
        DataKey::Initialized.into_val(&env),
        DataKey::Admin.into_val(&env),
        DataKey::TokenA.into_val(&env),
        DataKey::TokenB.into_val(&env),
        DataKey::ReserveA.into_val(&env),
        DataKey::ReserveB.into_val(&env),
        DataKey::TotalSupply.into_val(&env),
        DataKey::FeeBps.into_val(&env),
        DataKey::FeeRecipient.into_val(&env),
        DataKey::FeesA.into_val(&env),
        DataKey::FeesB.into_val(&env),
        DataKey::PriceOracleTimestamp.into_val(&env),
        DataKey::CumulativePrice.into_val(&env),
        DataKey::Balance.into_val(&env),
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

    let raw = symbol_short!("admin").into_val(&env);
    assert_ne!(variants[1], raw, "DataKey::Admin collides with raw 'admin'");
}
