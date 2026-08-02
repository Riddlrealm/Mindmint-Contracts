#![cfg(test)]
use soroban_sdk::vec;
use soroban_sdk::{symbol_short, xdr::ToXdr, Env, Symbol};

use crate::storage::DataKey;

/// Verifies that every `DataKey` variant serializes to a distinct storage key,
/// preventing the cross-proxy key collisions described in Issue #25.
#[test]
fn test_datakey_variant_keys_are_unique() {
    let env = Env::default();
    let sym = Symbol::new(&env, "XLMUSDC");

    let variants = vec![
        &env,
        DataKey::Config.to_xdr(&env),
        DataKey::History(sym.clone()).to_xdr(&env),
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

    let raw = symbol_short!("config").to_xdr(&env);
    assert_ne!(
        variants.get(0).unwrap(),
        raw,
        "DataKey::Config collides with raw 'config'"
    );
}
