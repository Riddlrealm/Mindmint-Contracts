#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_nft_lifecycle() {
    let env = Env::default();
    env.mock_all_auths(); // This mocks ALL require_auth() calls → admin and users can call without real signatures

    let contract_id = env.register_contract(None, AchievementNFT);
    let client = AchievementNFTClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    client.initialize(&admin);

    // Mark puzzle completed (admin calls it → mocked auth allows it)
    let puzzle_id = 101u32;
    client.mark_puzzle_completed(&user_a, &puzzle_id);

    let metadata = String::from_str(&env, "First Puzzle Master");
    let token_id = client.mint(&user_a, &puzzle_id, &metadata);

    assert_eq!(token_id, 1u32);
    assert_eq!(client.total_supply(), 1u32);
    assert_eq!(client.owner_of(&token_id), user_a);

    let achievement = client.get_achievement(&token_id).unwrap();
    assert_eq!(achievement.puzzle_id, puzzle_id);
    assert_eq!(achievement.owner, user_a);
    assert_eq!(achievement.metadata, metadata);

    // Transfer from user_a → user_b (mocked auth allows user_a.require_auth())
    client.transfer(&user_a, &user_b, &token_id);

    assert_eq!(client.owner_of(&token_id), user_b);
    assert_eq!(client.total_supply(), 1u32);

    let user_a_collection = client.get_collection(&user_a);
    let user_b_collection = client.get_collection(&user_b);
    assert_eq!(user_a_collection.len(), 0);
    assert_eq!(user_b_collection.len(), 1);
    assert_eq!(user_b_collection.get(0).unwrap(), token_id);

    // Burn from current owner (user_b)
    client.burn(&token_id);

    assert_eq!(client.total_supply(), 0u32);
    assert!(client.get_achievement(&token_id).is_none());
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_already_initialized() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AchievementNFT);
    let client = AchievementNFTClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);
    client.initialize(&admin); // should panic
}

#[test]
#[should_panic(expected = "Token does not exist")]
fn test_transfer_non_existent_token() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AchievementNFT);
    let client = AchievementNFTClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    client.initialize(&admin);

    client.transfer(&user_a, &user_b, &999u32); // non-existent
}

#[test]
#[should_panic(expected = "Puzzle not completed")]
fn test_mint_without_puzzle_completion() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AchievementNFT);
    let client = AchievementNFTClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);

    let metadata = String::from_str(&env, "Unauthorized Mint");
    let puzzle_id = 202u32;
    client.mint(&user, &puzzle_id, &metadata); // should panic
}

#[test]
#[should_panic(expected = "Cannot transfer to self")]
fn test_transfer_to_self() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AchievementNFT);
    let client = AchievementNFTClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);

    let puzzle_id = 303u32;
    client.mark_puzzle_completed(&user, &puzzle_id);

    let metadata = String::from_str(&env, "Self Transfer Test");
    let token_id = client.mint(&user, &puzzle_id, &metadata);

    client.transfer(&user, &user, &token_id); // should panic
}