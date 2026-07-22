#![cfg(test)]
use soroban_sdk::{symbol_short, Address, Env, IntoVal, Symbol, vec};

use crate::storage::DataKey;

/// Verifies that every `DataKey` variant serializes to a distinct storage key.
/// This is the core guard against the cross-proxy storage-key collisions
/// described in Issue #25: a documented, namespaced enum cannot accidentally
/// share a raw `symbol_short!` key (e.g. "admin"/"config") with another contract.
#[test]
fn test_datakey_variant_keys_are_unique() {
    let env = Env::default();
    let addr = Address::generate(&env);
    let sym = Symbol::new(&env, "role");

    let variants = vec![
        DataKey::Admin.into_val(&env),
        DataKey::EmergencyAdmin.into_val(&env),
        DataKey::Paused.into_val(&env),
        DataKey::UserRoles(addr.clone()).into_val(&env),
        DataKey::RolePermissions(sym.clone()).into_val(&env),
        DataKey::RoleParent(sym.clone()).into_val(&env),
        DataKey::AuditLogs.into_val(&env),
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

    // Sanity: an unrelated symbol-short key must never equal a data-key.
    let raw = symbol_short!("admin").into_val(&env);
    assert_ne!(variants[0], raw, "DataKey::Admin collides with raw 'admin'");
}
