#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env};
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let contract_id = env.register_stellar_asset_contract(admin.clone());
    (TokenClient::new(env, &contract_id), StellarAssetClient::new(env, &contract_id))
}

#[test]
fn test_delegation_flow() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let user3 = Address::generate(&env);
    let user4 = Address::generate(&env);
    
    let (token, token_admin) = create_token(&env, &admin);
    
    token_admin.mint(&user1, &1000);
    token_admin.mint(&user2, &2000);
    token_admin.mint(&user3, &3000);
    token_admin.mint(&user4, &4000);
    
    let contract_id = env.register_contract(None, DelegationContract);
    let client = DelegationContractClient::new(&env, &contract_id);
    
    client.initialize(&token.address);
    
    assert_eq!(client.get_voting_power(&user1), 1000);
    
    // User1 delegates to User2
    client.delegate(&user1, &user2);
    
    assert_eq!(client.get_voting_power(&user1), 0);
    assert_eq!(client.get_voting_power(&user2), 3000); // 2000 + 1000
    
    let chain1 = client.get_delegator_chain(&user1);
    assert_eq!(chain1.len(), 2);
    assert_eq!(chain1.get(0).unwrap(), user1);
    assert_eq!(chain1.get(1).unwrap(), user2);
    
    // User2 delegates to User3
    client.delegate(&user2, &user3);
    
    assert_eq!(client.get_voting_power(&user2), 0);
    assert_eq!(client.get_voting_power(&user3), 6000); // 3000 + 2000 + 1000(from user1)

    // User3 delegates to User4
    client.delegate(&user3, &user4);

    assert_eq!(client.get_voting_power(&user3), 0);
    assert_eq!(client.get_voting_power(&user4), 10000); // 4000+3000+2000+1000 (3 hops: u4 <- u3 <- u2 <- u1)

    let chain4 = client.get_delegator_chain(&user1);
    assert_eq!(chain4.len(), 4);
}

#[test]
#[should_panic(expected = "Circular delegation detected")]
fn test_circular_delegation() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let user3 = Address::generate(&env);
    
    let (token, _) = create_token(&env, &admin);
    
    let contract_id = env.register_contract(None, DelegationContract);
    let client = DelegationContractClient::new(&env, &contract_id);
    client.initialize(&token.address);
    
    client.delegate(&user1, &user2);
    client.delegate(&user2, &user3);
    
    // Should panic
    client.delegate(&user3, &user1);
}

#[test]
fn test_revoke_delegation() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&user1, &1000);
    token_admin.mint(&user2, &2000);
    
    let contract_id = env.register_contract(None, DelegationContract);
    let client = DelegationContractClient::new(&env, &contract_id);
    client.initialize(&token.address);
    
    client.delegate(&user1, &user2);
    assert_eq!(client.get_voting_power(&user1), 0);
    assert_eq!(client.get_voting_power(&user2), 3000);
    
    client.revoke_delegation(&user1);
    assert_eq!(client.get_voting_power(&user1), 1000);
    assert_eq!(client.get_voting_power(&user2), 2000);
}
