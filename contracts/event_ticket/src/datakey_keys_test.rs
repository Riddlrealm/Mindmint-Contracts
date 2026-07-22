#![cfg(test)]
use soroban_sdk::{symbol_short, Address, Env, IntoVal, vec};

use crate::storage::DataKey;

/// Verifies that every `DataKey` variant serializes to a distinct storage key,
/// preventing the cross-proxy key collisions described in Issue #25.
#[test]
fn test_datakey_variant_keys_are_unique() {
    let env = Env::default();
    let addr = Address::generate(&env);

    let variants = vec![
        DataKey::Config.into_val(&env),
        DataKey::Event(1u64).into_val(&env),
        DataKey::Ticket(1u64).into_val(&env),
        DataKey::HolderTickets(addr.clone()).into_val(&env),
        DataKey::Attendance(1u64).into_val(&env),
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

    let raw = symbol_short!("config").into_val(&env);
    assert_ne!(variants[0], raw, "DataKey::Config collides with raw 'config'");
}
