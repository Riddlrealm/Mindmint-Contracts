#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};

fn setup() -> (Env, Address, Address, DynamicNftContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, DynamicNftContract);
    let client = DynamicNftContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);

    (env, admin, user, client)
}

#[test]
fn mint_and_get() {
    let (env, _admin, user, client) = setup();
    env.ledger().set_timestamp(100);

    let token = client.mint(&user, &user, &String::from_str(&env, "meta_v1"), &String::from_str(&env, "traitA"));
    assert_eq!(token, 1);

    let nft = client.get_nft(&token).unwrap();
    assert_eq!(nft.owner, user);
    assert_eq!(nft.level, 1);
}

#[test]
fn time_evolution_changes_level() {
    let (env, _admin, user, client) = setup();
    env.ledger().set_timestamp(100);

    let token = client.mint(&user, &user, &String::from_str(&env, "meta_v1"), &String::from_str(&env, "traitA"));

    // not ready yet
    let res = std::panic::catch_unwind(|| client.evolve_time(&user, &token, &10u64));
    assert!(res.is_err());

    env.ledger().set_timestamp(200);
    client.evolve_time(&user, &token, &50u64);
    let nft = client.get_nft(&token).unwrap();
    assert_eq!(nft.level, 2);
}

#[test]
fn fuse_two_tokens() {
    let (env, _admin, user, client) = setup();
    env.ledger().set_timestamp(100);

    let a = client.mint(&user, &user, &String::from_str(&env, "a"), &String::from_str(&env, "A"));
    let b = client.mint(&user, &user, &String::from_str(&env, "b"), &String::from_str(&env, "B"));

    let fused = client.fuse(&user, &a, &b);
    assert_eq!(fused, 3);
    let nft = client.get_nft(&fused).unwrap();
    assert_eq!(nft.level, 2);
}
