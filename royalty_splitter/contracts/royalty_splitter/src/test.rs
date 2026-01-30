#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env, Map};

#[test]
fn test_distribution_and_withdraw() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    let mut splits = Map::new(&env);
    splits.set(alice.clone(), 6000);
    splits.set(bob.clone(), 4000);

    RoyaltySplitter::init(env.clone(), admin.clone(), splits, 10);

    RoyaltySplitter::distribute(env.clone(), 1000);

    // withdraw Alice
    alice.mock_auth(&env);
    RoyaltySplitter::withdraw(env.clone(), alice.clone());

    // withdraw Bob
    bob.mock_auth(&env);
    RoyaltySplitter::withdraw(env.clone(), bob.clone());
}

#[test]
#[should_panic]
fn test_invalid_split() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let alice = Address::generate(&env);

    let mut splits = Map::new(&env);
    splits.set(alice, 5000);

    RoyaltySplitter::init(env, admin, splits, 10);
}
