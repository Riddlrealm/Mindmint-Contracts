#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, BytesN, Env, contract, contractimpl};
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let contract_id = env.register_stellar_asset_contract(admin.clone());
    (TokenClient::new(env, &contract_id), StellarAssetClient::new(env, &contract_id))
}

#[contract]
pub struct MockOracle;

#[contractimpl]
impl MockOracle {
    pub fn verify(_env: Env, _puzzle_id: u32, hash: BytesN<32>) -> bool {
        let first_byte = hash.to_array()[0];
        first_byte == 1
    }
}

#[test]
fn test_flash_challenge_flow() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    let oracle_id = env.register_contract(None, MockOracle);
    
    let contract_id = env.register_contract(None, FlashChallengeContract);
    let client = FlashChallengeContractClient::new(&env, &contract_id);
    
    client.initialize(&admin, &token.address, &oracle_id);
    
    token_admin.mint(&admin, &10000);
    
    env.ledger().set_timestamp(1_000);
    
    let c_id = client.schedule(&1, &1000, &1_000, &15, &2);
    
    let player1 = Address::generate(&env);
    let player2 = Address::generate(&env);
    let player3 = Address::generate(&env);
    
    let bad_hash = BytesN::from_array(&env, &[0; 32]);
    let good_hash = BytesN::from_array(&env, &[1; 32]); 
    
    let res = client.try_submit_solution(&c_id, &player1, &bad_hash);
    assert!(res.is_err()); 
    
    client.submit_solution(&c_id, &player1, &good_hash);
    
    let (status, winners, remaining) = client.get_challenge(&c_id);
    assert_eq!(status, ChallengeStatus::Active);
    assert_eq!(winners.len(), 1);
    assert_eq!(remaining, 15 * 60);
    
    // player2 hits max winners
    client.submit_solution(&c_id, &player2, &good_hash);
    
    let (status2, winners2, _) = client.get_challenge(&c_id);
    assert_eq!(status2, ChallengeStatus::Completed);
    assert_eq!(winners2.len(), 2);
    
    assert_eq!(token.balance(&player1), 500);
    assert_eq!(token.balance(&player2), 500);
    
    let res3 = client.try_submit_solution(&c_id, &player3, &good_hash);
    assert!(res3.is_err());
}

#[test]
fn test_flash_challenge_expiry() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    let oracle_id = env.register_contract(None, MockOracle);
    
    let contract_id = env.register_contract(None, FlashChallengeContract);
    let client = FlashChallengeContractClient::new(&env, &contract_id);
    client.initialize(&admin, &token.address, &oracle_id);
    
    token_admin.mint(&admin, &10000);
    env.ledger().set_timestamp(1_000);
    
    let c_id = client.schedule(&1, &1000, &1_000, &15, &2);
    
    let player1 = Address::generate(&env);
    let good_hash = BytesN::from_array(&env, &[1; 32]);
    
    client.submit_solution(&c_id, &player1, &good_hash);
    
    env.ledger().set_timestamp(1_000 + 900 + 1);
    
    client.expire_challenge(&c_id);
    
    let (status, _, _) = client.get_challenge(&c_id);
    assert_eq!(status, ChallengeStatus::Expired);
    
    assert_eq!(token.balance(&player1), 500);
    assert_eq!(token.balance(&admin), 10000 - 1000 + 500); 
}
