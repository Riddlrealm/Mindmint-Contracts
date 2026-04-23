#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, token, Address, Env};

fn setup_token<'a>(
    env: &'a Env,
    admin: &'a Address,
) -> (Address, token::Client<'a>, token::StellarAssetClient<'a>) {
    let token_id = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_client = token::Client::new(env, &token_id);
    let token_admin_client = token::StellarAssetClient::new(env, &token_id);
    (token_id, token_client, token_admin_client)
}

#[test]
fn test_add_liquidity_and_get_pool() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let (token_a, token_a_client, token_a_admin) = setup_token(&env, &token_admin);
    let (token_b, token_b_client, token_b_admin) = setup_token(&env, &token_admin);

    token_a_admin.mint(&admin, &100_000);
    token_b_admin.mint(&admin, &100_000);

    let contract_id = env.register_contract(None, TokenSwapContract);
    let client = TokenSwapContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    client.create_pool(&admin, &1u32, &token_a, &token_b, &30u32);
    client.add_liquidity(&admin, &1u32, &10_000i128, &20_000i128);

    let pool = client.get_pool(&1u32);
    assert_eq!(pool.reserve_a, 10_000);
    assert_eq!(pool.reserve_b, 20_000);
    assert_eq!(pool.fee_bps, 30);
    assert_eq!(pool.total_swaps, 0);

    assert_eq!(token_a_client.balance(&contract_id), 10_000);
    assert_eq!(token_b_client.balance(&contract_id), 20_000);
}

#[test]
fn test_quote_matches_swap_output_and_fee_deducted() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let player = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let (token_a, token_a_client, token_a_admin) = setup_token(&env, &token_admin);
    let (token_b, token_b_client, token_b_admin) = setup_token(&env, &token_admin);

    token_a_admin.mint(&admin, &100_000);
    token_b_admin.mint(&admin, &100_000);
    token_a_admin.mint(&player, &10_000);

    let contract_id = env.register_contract(None, TokenSwapContract);
    let client = TokenSwapContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    client.create_pool(&admin, &1u32, &token_a, &token_b, &100u32); // 1%
    client.add_liquidity(&admin, &1u32, &10_000i128, &10_000i128);

    let amount_in: i128 = 1_000;

    // quote is net output (after fee) and must match swap return
    let quote = client.quote_swap(&1u32, &token_a, &amount_in);
    let out = client.swap(&player, &1u32, &token_a, &amount_in);
    assert_eq!(out, quote);

    // fee should be deducted from gross output and accumulated as fees_b
    let pool = client.get_pool(&1u32);
    assert_eq!(pool.total_swaps, 1);

    assert_eq!(token_a_client.balance(&player), 9_000);
    assert!(token_b_client.balance(&player) > 0);
    assert_eq!(token_b_client.balance(&player), out);

    assert!(pool.fees_b > 0);
}

#[test]
fn test_slippage_calculation_reserves_update() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let player1 = Address::generate(&env);
    let player2 = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let (token_a, _, token_a_admin) = setup_token(&env, &token_admin);
    let (token_b, _, token_b_admin) = setup_token(&env, &token_admin);

    token_a_admin.mint(&admin, &100_000);
    token_b_admin.mint(&admin, &100_000);
    token_a_admin.mint(&player1, &10_000);
    token_a_admin.mint(&player2, &10_000);

    let contract_id = env.register_contract(None, TokenSwapContract);
    let client = TokenSwapContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    client.create_pool(&admin, &1u32, &token_a, &token_b, &0u32);
    client.add_liquidity(&admin, &1u32, &10_000i128, &10_000i128);

    let out1 = client.swap(&player1, &1u32, &token_a, &1_000i128);
    let out2 = client.swap(&player2, &1u32, &token_a, &1_000i128);

    // second trader gets worse price due to slippage
    assert!(out2 < out1);

    let pool = client.get_pool(&1u32);
    assert_eq!(pool.reserve_a, 12_000);
    assert_eq!(pool.reserve_b, 10_000 - (out1 + out2));
}

#[test]
#[should_panic(expected = "Insufficient liquidity")]
fn test_insufficient_liquidity_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let player = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let (token_a, _, token_a_admin) = setup_token(&env, &token_admin);
    let (token_b, _, token_b_admin) = setup_token(&env, &token_admin);

    token_a_admin.mint(&admin, &100_000);
    token_b_admin.mint(&admin, &100_000);
    token_a_admin.mint(&player, &10_000);

    let contract_id = env.register_contract(None, TokenSwapContract);
    let client = TokenSwapContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    client.create_pool(&admin, &1u32, &token_a, &token_b, &0u32);

    // no liquidity added
    client.swap(&player, &1u32, &token_a, &1_000i128);
}

#[test]
fn test_claim_fees() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let player = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let (token_a, token_a_client, token_a_admin) = setup_token(&env, &token_admin);
    let (token_b, token_b_client, token_b_admin) = setup_token(&env, &token_admin);

    token_a_admin.mint(&admin, &100_000);
    token_b_admin.mint(&admin, &100_000);
    token_a_admin.mint(&player, &10_000);

    let contract_id = env.register_contract(None, TokenSwapContract);
    let client = TokenSwapContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    client.create_pool(&admin, &1u32, &token_a, &token_b, &200u32); // 2%
    client.add_liquidity(&admin, &1u32, &10_000i128, &10_000i128);

    let _out = client.swap(&player, &1u32, &token_a, &1_000i128);

    let pool_before = client.get_pool(&1u32);
    let admin_b_before = token_b_client.balance(&admin);
    let (fees_a, fees_b) = client.claim_fees(&admin, &1u32);

    assert_eq!(fees_a, 0);
    assert!(fees_b > 0);
    assert_eq!(token_b_client.balance(&admin), admin_b_before + fees_b);

    let pool = client.get_pool(&1u32);
    assert_eq!(pool.fees_a, 0);
    assert_eq!(pool.fees_b, 0);

    // reserves should be reduced by the fees paid out
    assert_eq!(pool.reserve_a, pool_before.reserve_a - fees_a);
    assert_eq!(pool.reserve_b, pool_before.reserve_b - fees_b);
}
