#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::Client as TokenClient,
    token::StellarAssetClient,
    Address, Env,
};

fn create_token<'a>(env: &Env, admin: &Address) -> (Address, TokenClient<'a>, StellarAssetClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let addr = sac.address();
    (
        addr.clone(),
        TokenClient::new(env, &addr),
        StellarAssetClient::new(env, &addr),
    )
}

fn setup<'a>() -> (
    Env,
    PuzzlePoolStakingContractClient<'a>,
    Address,
    Address,
    Address,
    Address,
    TokenClient<'a>,
    StellarAssetClient<'a>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let (token_addr, token_client, token_admin_client) = create_token(&env, &token_admin);

    let contract_id = env.register_contract(None, PuzzlePoolStakingContract);
    let client = PuzzlePoolStakingContractClient::new(&env, &contract_id);

    let week = 7 * 24 * 60 * 60u64;
    let lock = 7 * 24 * 60 * 60u64;
    let solver_bps = 7000u32;

    client.initialize(&admin, &oracle, &token_addr, &week, &lock, &solver_bps);

    (
        env,
        client,
        admin,
        oracle,
        token_admin,
        token_addr,
        token_client,
        token_admin_client,
    )
}

#[test]
fn test_stake_and_balance() {
    let (env, client, _, _, _, _, token_client, token_admin) = setup();
    let staker = Address::generate(&env);

    env.ledger().set_timestamp(1_000);
    token_admin.mint(&staker, &100_000_000_000);

    client.stake(&staker, &50_000_000_000);

    assert_eq!(client.get_total_staked(), 50_000_000_000);
    let pos = client.get_stake(&staker).0;
    assert_eq!(pos.amount, 50_000_000_000);
    assert_eq!(pos.staked_at, 1_000);
    assert_eq!(token_client.balance(&staker), 50_000_000_000);
}

#[test]
#[should_panic(expected = "Stake still locked")]
fn test_unstake_lock_enforced() {
    let (env, client, _, _, _, _, _, token_admin) = setup();
    let staker = Address::generate(&env);

    env.ledger().set_timestamp(1_000);
    token_admin.mint(&staker, &100_000_000_000);
    client.stake(&staker, &10_000_000_000);

    env.ledger().set_timestamp(1_000 + 1);
    client.unstake(&staker, &1_000_000_000);
}

#[test]
fn test_unstake_after_lock_partial() {
    let (env, client, _, _, _, _, token_client, token_admin) = setup();
    let staker = Address::generate(&env);

    let t0 = 1_000_000u64;
    env.ledger().set_timestamp(t0);
    token_admin.mint(&staker, &100_000_000_000);
    client.stake(&staker, &10_000_000_000);

    let lock = 7 * 24 * 60 * 60u64;
    env.ledger().set_timestamp(t0 + lock);

    client.unstake(&staker, &4_000_000_000);
    assert_eq!(client.get_total_staked(), 6_000_000_000);
    assert_eq!(token_client.balance(&staker), 94_000_000_000);
}

#[test]
fn test_record_solve_weighted_and_epoch_close_and_claim() {
    let (env, client, admin, oracle, _, _, token_client, token_admin) = setup();
    let player_a = Address::generate(&env);
    let player_b = Address::generate(&env);

    env.ledger().set_timestamp(10_000);
    token_admin.mint(&admin, &1_000_000_000_000);
    token_admin.mint(&player_a, &500_000_000_000);
    token_admin.mint(&player_b, &500_000_000_000);

    // Stakers split the pool: A = 30, B = 70 (same token units)
    client.stake(&player_a, &30_000_000_000);
    client.stake(&player_b, &70_000_000_000);

    // Weighted solves: A difficulty 2 + 1 = 3 weight, B difficulty 10 = 10 weight -> total 13
    client.record_solve(&oracle, &player_a, &2);
    client.record_solve(&oracle, &player_a, &1);
    client.record_solve(&oracle, &player_b, &10);

    let week = 7 * 24 * 60 * 60u64;
    env.ledger().set_timestamp(10_000 + week + 1);

    client.fund_pool(&admin, &1_000_000_000);
    assert_eq!(client.get_reward_pool(), 1_000_000_000);

    client.close_epoch(&admin, &1_000_000_000);

    assert_eq!(client.get_current_epoch_id(), 1);
    let e0 = client.get_epoch(&0);
    assert!(e0.distributed);
    assert_eq!(e0.total_solves, 13);
    assert_eq!(e0.reward_budget, 1_000_000_000);
    assert_eq!(e0.total_staked_snapshot, 100_000_000_000);

    // 70% solver pool = 700_000_000; 30% staker pool = 300_000_000
    // A solver: 700_000_000 * 3 / 13 = 161_538_461 (floor)
    // B solver: 700_000_000 * 10 / 13 = 538_461_538
    // A staker: 300_000_000 * 30 / 100 = 90_000_000
    // B staker: 300_000_000 * 70 / 100 = 210_000_000

    let prev_a = token_client.balance(&player_a);
    let claimed_a = client.claim_epoch_reward(&player_a, &0);
    let prev_b = token_client.balance(&player_b);
    let claimed_b = client.claim_epoch_reward(&player_b, &0);

    assert_eq!(token_client.balance(&player_a) - prev_a, claimed_a);
    assert_eq!(token_client.balance(&player_b) - prev_b, claimed_b);

    let expected_a = (700_000_000i128 * 3) / 13 + 90_000_000;
    let expected_b = (700_000_000i128 * 10) / 13 + 210_000_000;
    assert_eq!(claimed_a, expected_a);
    assert_eq!(claimed_b, expected_b);

    assert_eq!(client.preview_claim(&player_a, &0), 0);
    assert_eq!(client.preview_claim(&player_b, &0), 0);
}

#[test]
#[should_panic(expected = "Already claimed")]
fn test_claim_idempotent() {
    let (env, client, admin, oracle, _, _, _, token_admin) = setup();
    let player = Address::generate(&env);

    env.ledger().set_timestamp(100);
    token_admin.mint(&admin, &500_000_000_000);
    token_admin.mint(&player, &100_000_000_000);

    client.stake(&player, &10_000_000_000);
    client.record_solve(&oracle, &player, &5);

    let week = 7 * 24 * 60 * 60u64;
    env.ledger().set_timestamp(100 + week + 1);
    client.fund_pool(&admin, &100_000_000);
    client.close_epoch(&admin, &100_000_000);

    client.claim_epoch_reward(&player, &0);
    client.claim_epoch_reward(&player, &0);
}

#[test]
fn test_get_stake_lists_unclaimed_epochs() {
    let (env, client, admin, oracle, _, _, _, token_admin) = setup();
    let player = Address::generate(&env);

    env.ledger().set_timestamp(1);
    token_admin.mint(&admin, &900_000_000_000);
    token_admin.mint(&player, &200_000_000_000);
    client.stake(&player, &50_000_000_000);
    client.record_solve(&oracle, &player, &3);

    let week = 7 * 24 * 60 * 60u64;
    env.ledger().set_timestamp(1 + week + 1);
    client.fund_pool(&admin, &50_000_000);
    client.close_epoch(&admin, &50_000_000);

    let (_, unclaimed) = client.get_stake(&player);
    assert_eq!(unclaimed.len(), 1);
    assert_eq!(unclaimed.get(0).unwrap(), 0);
}
