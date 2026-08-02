#![cfg(test)]
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, vec, xdr::ToXdr, Address, Env, Symbol};

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
        &env,
        DataKey::Admin.to_xdr(&env),
        DataKey::EmergencyAdmin.to_xdr(&env),
        DataKey::Paused.to_xdr(&env),
        DataKey::UserRoles(addr.clone()).to_xdr(&env),
        DataKey::RolePermissions(sym.clone()).to_xdr(&env),
        DataKey::RoleParent(sym.clone()).to_xdr(&env),
        DataKey::AuditLogs.to_xdr(&env),
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

    // Sanity: an unrelated symbol-short key must never equal a data-key.
    let raw = symbol_short!("admin").to_xdr(&env);
    assert_ne!(
        variants.get(0).unwrap(),
        raw,
        "DataKey::Admin collides with raw 'admin'"
    );
}
