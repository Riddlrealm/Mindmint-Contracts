#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events}, 
    Address, Env, String, symbol_short, IntoVal
};

#[test]
fn test_nft_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AchievementNFT);
    let client = AchievementNFTClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    client.initialize(&admin);

    // 1. Test Minting
    let metadata = String::from_str(&env, "Master Puzzler");
    let token_id_1 = client.mint(&user_a, &101, &metadata);
    let token_id_2 = client.mint(&user_a, &102, &metadata);
    
    assert_eq!(token_id_1, 1);
    assert_eq!(token_id_2, 2);

    // 2. Test Transfer
    client.transfer(&user_a, &user_b, &token_id_1);
    assert_eq!(client.owner_of(&token_id_1), user_b);

    // 3. Test Burn
    client.burn(&token_id_2);
    assert_eq!(client.total_supply(), 1);

    // 4. Verify Events (Uses IntoVal trait)
    let last_event = env.events().all().last().unwrap();
    assert_eq!(
        last_event,
        (
            contract_id.clone(),
            (symbol_short!("burn"), user_a.clone()).into_val(&env),
            token_id_2.into_val(&env)
        )
    );
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_already_initialized() {
    let env = Env::default();
    let contract_id = env.register_contract(None, AchievementNFT);
    let client = AchievementNFTClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);
    client.initialize(&admin);
}

#[test]
#[should_panic(expected = "Not the owner")]
fn test_unauthorized_transfer() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, AchievementNFT);
    let client = AchievementNFTClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let _user_b = Address::generate(&env);
    let hacker = Address::generate(&env);

    client.initialize(&admin);
    let token_id = client.mint(&user_a, &1, &String::from_str(&env, "test"));

    // This will panic due to ownership check
    client.transfer(&hacker, &hacker, &token_id);
}

#[test]
fn test_get_non_existent_achievement() {
    let env = Env::default();
    let contract_id = env.register_contract(None, AchievementNFT);
    let client = AchievementNFTClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert!(client.get_achievement(&99).is_none());
}