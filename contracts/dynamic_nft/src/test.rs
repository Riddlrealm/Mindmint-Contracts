#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup<'a>(
    env: &'a Env,
) -> (
    Address,
    Address,
    Address,
    DynamicNftContractClient<'a>,
) {
    env.mock_all_auths();

    let admin = Address::generate(env);
    let oracle = Address::generate(env);
    let user = Address::generate(env);

    let contract_id = env.register_contract(None, DynamicNftContract);
    let client = DynamicNftContractClient::new(env, &contract_id);

    client.initialize(&admin, &oracle);

    (admin, oracle, user, client)
}

#[test]
fn test_initialize() {
    let env = Env::default();
    let _ = setup(&env);
}

#[test]
fn test_mint() {
    let env = Env::default();
    let (_admin, _oracle, user, client) = setup(&env);

    let token_id = client.mint(&user);
    assert_eq!(token_id, 1);

    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.token_id, 1);
    assert_eq!(nft.owner, user);
    assert_eq!(nft.level, 1);
    assert_eq!(nft.evolution_stage, 1);
    assert_eq!(nft.xp, 0);
    assert_eq!(nft.metadata_uri, String::from_str(&env, "ipfs://base-metadata"));
}

#[test]
fn test_add_xp() {
    let env = Env::default();
    let (_admin, oracle, user, client) = setup(&env);
    let token_id = client.mint(&user);

    client.add_xp(&oracle, &token_id, &100);

    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.xp, 100);
}

#[test]
#[should_panic(expected = "Not oracle")]
fn test_add_xp_unauthorized() {
    let env = Env::default();
    let (_admin, _oracle, user, client) = setup(&env);
    let unauthorized = Address::generate(&env);
    let token_id = client.mint(&user);

    client.add_xp(&unauthorized, &token_id, &100);
}

#[test]
fn test_evolution_rule_management() {
    let env = Env::default();
    let (admin, _oracle, _user, client) = setup(&env);

    let metadata_uri = String::from_str(&env, "ipfs://level2-metadata");
    client.add_evolution_rule(&admin, &100, &2, &metadata_uri);

    let rules = client.get_evolution_rules();
    assert_eq!(rules.len(), 1);

    let rule = rules.get(100u64).unwrap();
    assert_eq!(rule.min_xp, 100);
    assert_eq!(rule.new_level, 2);
    assert_eq!(rule.new_metadata_uri, metadata_uri);

    client.remove_evolution_rule(&admin, &100);

    let rules = client.get_evolution_rules();
    assert_eq!(rules.len(), 0);
}

#[test]
fn test_evolution_trigger() {
    let env = Env::default();
    let (admin, oracle, user, client) = setup(&env);
    let token_id = client.mint(&user);

    let metadata_uri = String::from_str(&env, "ipfs://level2-metadata");
    client.add_evolution_rule(&admin, &100, &2, &metadata_uri);

    client.add_xp(&oracle, &token_id, &150);

    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.level, 2);
    assert_eq!(nft.evolution_stage, 2);
    assert_eq!(nft.metadata_uri, metadata_uri);
    assert_eq!(nft.xp, 150);
}

#[test]
fn test_manual_evolution() {
    let env = Env::default();
    let (admin, oracle, user, client) = setup(&env);
    let token_id = client.mint(&user);

    let metadata_uri = String::from_str(&env, "ipfs://level2-metadata");
    client.add_evolution_rule(&admin, &100, &2, &metadata_uri);

    client.add_xp(&oracle, &token_id, &50);

    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.level, 1);

    client.add_xp(&oracle, &token_id, &50);

    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.level, 2);
}

#[test]
#[should_panic(expected = "NFT is soulbound until level 3")]
fn test_transfer_soulbound_panics() {
    let env = Env::default();
    let (_admin, _oracle, user, client) = setup(&env);
    let recipient = Address::generate(&env);
    let token_id = client.mint(&user);
    client.transfer(&user, &recipient, &token_id);
}

#[test]
fn test_transfer_after_level3() {
    let env = Env::default();
    let (admin, oracle, user, client) = setup(&env);
    let recipient = Address::generate(&env);
    let token_id = client.mint(&user);

    let metadata_uri2 = String::from_str(&env, "ipfs://level2-metadata");
    let metadata_uri3 = String::from_str(&env, "ipfs://level3-metadata");
    client.add_evolution_rule(&admin, &100, &2, &metadata_uri2);
    client.add_evolution_rule(&admin, &200, &3, &metadata_uri3);

    client.add_xp(&oracle, &token_id, &200);

    client.transfer(&user, &recipient, &token_id);

    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.owner, recipient);
}

#[test]
fn test_max_level_cap() {
    let env = Env::default();
    let (admin, oracle, user, client) = setup(&env);
    let token_id = client.mint(&user);

    client.add_evolution_rule(&admin, &100, &2, &String::from_str(&env, "ipfs://level2"));
    client.add_evolution_rule(&admin, &200, &3, &String::from_str(&env, "ipfs://level3"));
    client.add_evolution_rule(&admin, &300, &4, &String::from_str(&env, "ipfs://level4"));
    client.add_evolution_rule(&admin, &400, &5, &String::from_str(&env, "ipfs://level5"));

    client.add_xp(&oracle, &token_id, &500);

    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.level, 5);

    client.add_xp(&oracle, &token_id, &100);

    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.level, 5);
}

#[test]
#[should_panic(expected = "Not oracle")]
fn test_old_oracle_rejected_after_update() {
    let env = Env::default();
    let (admin, oracle, user, client) = setup(&env);
    let new_oracle = Address::generate(&env);
    let token_id = client.mint(&user);
    client.add_xp(&oracle, &token_id, &50);
    client.update_oracle(&admin, &new_oracle);
    client.add_xp(&oracle, &token_id, &50);
}

#[test]
fn test_oracle_update_new_oracle_works() {
    let env = Env::default();
    let (admin, oracle, user, client) = setup(&env);
    let new_oracle = Address::generate(&env);
    let token_id = client.mint(&user);
    client.add_xp(&oracle, &token_id, &50);
    client.update_oracle(&admin, &new_oracle);
    client.add_xp(&new_oracle, &token_id, &50);
    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.xp, 100);
}

#[test]
fn test_event_emission() {
    let env = Env::default();
    let (admin, oracle, user, client) = setup(&env);

    let token_id = client.mint(&user);

    client.add_xp(&oracle, &token_id, &100);

    let metadata_uri = String::from_str(&env, "ipfs://level2-metadata");
    client.add_evolution_rule(&admin, &50, &2, &metadata_uri);

    client.add_xp(&oracle, &token_id, &50);

    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.level, 2);
}

#[test]
#[should_panic(expected = "Only owner can trigger evolution")]
fn test_evolve_unauthorized() {
    let env = Env::default();
    let (_admin, _oracle, user, client) = setup(&env);
    let unauthorized = Address::generate(&env);
    let token_id = client.mint(&user);

    client.evolve(&unauthorized, &token_id);
}

#[test]
#[should_panic(expected = "Not admin")]
fn test_evolution_rule_unauthorized() {
    let env = Env::default();
    let (_admin, _oracle, _user, client) = setup(&env);
    let unauthorized = Address::generate(&env);

    client.add_evolution_rule(
        &unauthorized,
        &100,
        &2,
        &String::from_str(&env, "ipfs://level2"),
    );
}
