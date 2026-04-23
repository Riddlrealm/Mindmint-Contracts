#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, contract, contractimpl};
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let contract_id = env.register_stellar_asset_contract_v2(admin.clone());
    (TokenClient::new(env, &contract_id.address()), StellarAssetClient::new(env, &contract_id.address()))
}

#[test]
fn test_liquidity_mining_flow() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let (lp_token, lp_admin) = create_token(&env, &admin);
    let (reward_token, reward_admin) = create_token(&env, &admin);
    
    let contract_id = env.register_contract(None, LiquidityMiningContract);
    let client = LiquidityMiningContractClient::new(&env, &contract_id);
    
    client.initialize(&admin, &lp_token.address, &reward_token.address);
    
    let player1 = Address::generate(&env);
    let player2 = Address::generate(&env);
    
    lp_admin.mint(&player1, &1000);
    lp_admin.mint(&player2, &3000);
    reward_admin.mint(&admin, &100_000);
    
    // Player 1 stakes 1000
    env.ledger().set_timestamp(1_000);
    client.stake_lp(&player1, &1000);
    
    // Admin opens Epoch 1 with 10_000 rewards
    client.fund_and_open_epoch(&10_000);
    
    // Player 2 stakes 3000 at the end
    client.stake_lp(&player2, &3000);
    
    // Closes Epoch 1
    client.close_epoch();
    
    // Total staked globally was 4000 at the end of epoch 1.
    // P1 = 1000/4000 = 25% = 2500 tokens
    // P2 = 3000/4000 = 75% = 7500 tokens
    
    client.claim_mining_reward(&player1, &1);
    assert_eq!(reward_token.balance(&player1), 2500);
    
    client.claim_mining_reward(&player2, &1);
    assert_eq!(reward_token.balance(&player2), 7500);
    
    // Player 1 cannot unstake without wait
    let res = client.try_unstake_lp(&player1, &1000);
    assert!(res.is_err());
    
    // Move time forward 24h+
    env.ledger().set_timestamp(1_000 + 86401);
    client.unstake_lp(&player1, &1000);
    assert_eq!(lp_token.balance(&player1), 1000);
}

#[test]
fn test_double_claim_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let (lp_token, lp_admin) = create_token(&env, &admin);
    let (reward_token, reward_admin) = create_token(&env, &admin);
    
    let contract_id = env.register_contract(None, LiquidityMiningContract);
    let client = LiquidityMiningContractClient::new(&env, &contract_id);
    client.initialize(&admin, &lp_token.address, &reward_token.address);
    let p = Address::generate(&env);
    lp_admin.mint(&p, &1000);
    reward_admin.mint(&admin, &10_000);
    
    env.ledger().set_timestamp(1_000);
    client.stake_lp(&p, &1000);
    client.fund_and_open_epoch(&10_000);
    env.ledger().set_timestamp(2_000);
    client.close_epoch();
    
    client.claim_mining_reward(&p, &1);
    let curr_balance = reward_token.balance(&p);
    
    // Cannot claim again
    let res = client.try_claim_mining_reward(&p, &1);
    assert!(res.is_err());
    
    assert_eq!(reward_token.balance(&p), curr_balance);
}

#[test]
fn test_requires_pending_claims_to_be_done_before_unstake() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let (lp_token, lp_admin) = create_token(&env, &admin);
    let (reward_token, reward_admin) = create_token(&env, &admin);
    
    let contract_id = env.register_contract(None, LiquidityMiningContract);
    let client = LiquidityMiningContractClient::new(&env, &contract_id);
    client.initialize(&admin, &lp_token.address, &reward_token.address);
    let p = Address::generate(&env);
    lp_admin.mint(&p, &1000);
    reward_admin.mint(&admin, &10_000);
    
    env.ledger().set_timestamp(1_000);
    client.stake_lp(&p, &1000);
    client.fund_and_open_epoch(&10_000);
    client.close_epoch();
    
    // Fast forward past cooldown
    env.ledger().set_timestamp(1_000 + 86401);
    
    // Unstaking should fail because player hasn't claimed closed epoch 1
    let res = client.try_unstake_lp(&p, &500);
    assert!(res.is_err());
    
    client.claim_mining_reward(&p, &1);
    
    // Now they can
    client.unstake_lp(&p, &500);
    assert_eq!(lp_token.balance(&p), 500);
}
