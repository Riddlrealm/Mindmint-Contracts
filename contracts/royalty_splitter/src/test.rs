#![cfg(test)]

use super::*;
use soroban_sdk::{Env, Map, Address};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_distribution_and_withdraw() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(RoyaltySplitter, ());

    let admin = Address::generate(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    let mut splits = Map::new(&env);
    splits.set(alice.clone(), 6000);
    splits.set(bob.clone(), 4000);

    env.as_contract(&contract_id, || {
        RoyaltySplitter::init(env.clone(), admin.clone(), splits, 10);
        RoyaltySplitter::distribute(env.clone(), 1000);
        RoyaltySplitter::withdraw(env.clone(), alice.clone());
        RoyaltySplitter::withdraw(env.clone(), bob.clone());
    });
}

#[test]
#[should_panic]
fn test_invalid_split() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(RoyaltySplitter, ());

    let admin = Address::generate(&env);
    let alice = Address::generate(&env);

    let mut splits = Map::new(&env);
    splits.set(alice, 5000);

    env.as_contract(&contract_id, || {
        RoyaltySplitter::init(env.clone(), admin, splits, 10);
    });
}
