#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, Address, Env};

fn setup_test(env: &Env) -> (BountyContractClient<'_>, Address, token::Client<'_>) {
    let admin = Address::generate(env);
    let contract_id = env.register_contract(None, BountyContract);
    let client = BountyContractClient::new(env, &contract_id);
    client.initialize(&admin);

    let token_admin = Address::generate(env);
    let token_contract_id = env.register_stellar_asset_contract(token_admin.clone());
    let token_client = token::Client::new(env, &token_contract_id);

    (client, admin, token_client)
}

#[test]
fn test_bounty_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, token_client) = setup_test(&env);
    let token_asset_client = token::StellarAssetClient::new(&env, &token_client.address);

    let creator = Address::generate(&env);
    let solver = Address::generate(&env);

    // Mint tokens to creator
    token_asset_client.mint(&creator, &1000);
    assert_eq!(token_client.balance(&creator), 1000);

    // 1. Create Bounty
    let bounty_id = client.create_bounty(&creator, &token_client.address, &500, &Some(1), &3600);
    assert_eq!(bounty_id, 1);
    assert_eq!(token_client.balance(&creator), 500);
    assert_eq!(token_client.balance(&client.address), 500);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.status, BountyStatus::Open);

    // 2. Accept Bounty
    client.accept_bounty(&solver, &bounty_id);
    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.status, BountyStatus::Accepted);
    assert_eq!(bounty.solver, Some(solver.clone()));

    // 3. Submit Solution
    client.submit_solution(&solver, &bounty_id);
    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.status, BountyStatus::Submitted);

    // 4. Approve Submission
    client.approve_submission(&creator, &bounty_id);
    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.status, BountyStatus::Completed);
    
    // Check payouts
    assert_eq!(token_client.balance(&solver), 500);
    assert_eq!(token_client.balance(&client.address), 0);
}

#[test]
fn test_cancel_bounty() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, token_client) = setup_test(&env);

    let creator = Address::generate(&env);
    let token_asset_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_asset_client.mint(&creator, &1000);

    let bounty_id = client.create_bounty(&creator, &token_client.address, &500, &None, &3600);
    
    // Cancel while open
    client.cancel_bounty(&creator, &bounty_id);
    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.status, BountyStatus::Cancelled);
    assert_eq!(token_client.balance(&creator), 1000);
}

#[test]
#[should_panic(expected = "Cannot cancel at this stage or not yet expired")]
fn test_cancel_accepted_fails_before_expiration() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, token_client) = setup_test(&env);

    let creator = Address::generate(&env);
    let solver = Address::generate(&env);
    let token_asset_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_asset_client.mint(&creator, &1000);

    let bounty_id = client.create_bounty(&creator, &token_client.address, &500, &None, &3600);
    client.accept_bounty(&solver, &bounty_id);

    client.cancel_bounty(&creator, &bounty_id);
}

#[test]
fn test_dispute_resolution() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, token_client) = setup_test(&env);

    let creator = Address::generate(&env);
    let solver = Address::generate(&env);
    let token_asset_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_asset_client.mint(&creator, &1000);

    let bounty_id = client.create_bounty(&creator, &token_client.address, &500, &None, &3600);
    client.accept_bounty(&solver, &bounty_id);
    client.submit_solution(&solver, &bounty_id);

    client.dispute_bounty(&creator, &bounty_id);
    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.status, BountyStatus::Disputed);

    // Resolve: 300 to solver, 200 to creator
    client.resolve_dispute(&admin, &bounty_id, &300);
    
    assert_eq!(token_client.balance(&solver), 300);
    assert_eq!(token_client.balance(&creator), 500 + 200); // initial 500 left + 200 refund
}

#[test]
fn test_marketplace_discovery() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, token_client) = setup_test(&env);

    let creator = Address::generate(&env);
    let token_asset_client = token::StellarAssetClient::new(&env, &token_client.address);
    token_asset_client.mint(&creator, &5000);

    for i in 0..5 {
        client.create_bounty(&creator, &token_client.address, &100, &Some(i), &3600);
    }

    let active = client.get_active_bounties(&0, &10);
    assert_eq!(active.len(), 5);

    // Cancel one
    client.cancel_bounty(&creator, &1);
    let active = client.get_active_bounties(&0, &10);
    assert_eq!(active.len(), 4);

    // Complete one
    let solver = Address::generate(&env);
    client.accept_bounty(&solver, &2);
    client.submit_solution(&solver, &2);
    client.approve_submission(&creator, &2);

    let active = client.get_active_bounties(&0, &10);
    assert_eq!(active.len(), 3);
}
