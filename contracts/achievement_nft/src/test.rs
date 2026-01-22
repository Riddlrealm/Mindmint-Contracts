#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Events}, Address, Env, String, symbol_short, Vec};

#[test]
fn test_nft_lifecycle() {
    // 1. Setup the Soroban Environment
    let env = Env::default();
    env.mock_all_auths();

    // 2. Register the AchievementNFT contract and create a client
    let contract_id = env.register_contract(None, AchievementNFT);
    let client = AchievementNFTClient::new(&env, &contract_id);

    // 3. Generate test addresses
    let admin = Address::generate(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    // 4. Initialize
    client.initialize(&admin);

    // 5. Test Minting and Enumeration
    let metadata = String::from_str(&env, "Master Puzzler");
    let token_id_1 = client.mint(&user_a, &101, &metadata);
    let token_id_2 = client.mint(&user_a, &102, &metadata);
    
    assert_eq!(token_id_1, 1);
    assert_eq!(token_id_2, 2);
    assert_eq!(client.total_supply(), 2);

    // Verify User A's collection contains both IDs
    let collection_a = client.get_collection(&user_a);
    assert_eq!(collection_a.len(), 2);
    assert_eq!(collection_a.get(0).unwrap(), 1);
    assert_eq!(collection_a.get(1).unwrap(), 2);

    // 6. Test Transfer and Enumeration Update
    // Transfer token 1 from A to B
    client.transfer(&user_a, &user_b, &token_id_1);
    
    assert_eq!(client.owner_of(&token_id_1), user_b);
    
    // Check collection updates
    let new_collection_a = client.get_collection(&user_a);
    let collection_b = client.get_collection(&user_b);
    
    assert_eq!(new_collection_a.len(), 1); // Only token 2 remains
    assert_eq!(new_collection_a.get(0).unwrap(), 2);
    
    assert_eq!(collection_b.len(), 1); // Now owns token 1
    assert_eq!(collection_b.get(0).unwrap(), 1);

    // 7. Test Burning and Enumeration Update
    client.burn(&token_id_2);
    
    assert_eq!(client.total_supply(), 1);
    let final_collection_a = client.get_collection(&user_a);
    assert_eq!(final_collection_a.len(), 0);

    // 8. Verify Events
    // We expect 2 mints, 1 transfer, and 1 burn event
    let last_event = env.events().all().last().unwrap();
    // The event published in burn is: (symbol_short!("burn"), achievement.owner), token_id
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
    let user_b = Address::generate(&env);
    let hacker = Address::generate(&env);

    client.initialize(&admin);
    let token_id = client.mint(&user_a, &1, &String::from_str(&env, "test"));

    // Hacker tries to transfer User A's token to themselves
    // This will panic because require_auth is called on 'from' (user_a)
    // but the test context would fail if hacker tries to sign for user_a.
    // Or, if hacker signs for hacker, the owner check "achievement.owner != from" fails.
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