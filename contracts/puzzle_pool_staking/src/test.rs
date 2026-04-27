#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::Client as TokenClient,
    token::StellarAssetClient,
    Address, Env,
};

fn create_token_contract<'a>(env: &Env, admin: &Address) -> (Address, TokenClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let address = sac.address();
    (address.clone(), TokenClient::new(env, &address))
}

fn setup_contract(env: &Env) -> (
    PuzzlePoolStakingClient,
    Address,
    Address,
    Address,
    TokenClient,
    TokenClient,
    StellarAssetClient,
    StellarAssetClient,
) {
    let admin = Address::generate(env);
    let staker = Address::generate(env);
    let oracle = Address::generate(env);
    let token_admin = Address::generate(env);

    // Create staking and reward tokens
    let (staking_token_addr, staking_token_client) = create_token_contract(env, &token_admin);
    let (reward_token_addr, reward_token_client) = create_token_contract(env, &token_admin);

    let staking_admin_client = StellarAssetClient::new(env, &staking_token_addr);
    let reward_admin_client = StellarAssetClient::new(env, &reward_token_addr);

    // Register contract
    let contract_id = env.register_contract(None, PuzzlePoolStaking);
    let client = PuzzlePoolStakingClient::new(env, &contract_id);

    // Initialize with 7-day epoch duration
    let epoch_duration = 7 * 24 * 60 * 60u64; // 7 days

    client.initialize(
        &admin,
        &staking_token_addr,
        &reward_token_addr,
        &oracle,
        &epoch_duration,
    );

    (
        client,
        admin,
        staker,
        oracle,
        staking_token_client,
        reward_token_client,
        staking_admin_client,
        reward_admin_client,
    )
}

#[test]
fn test_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, oracle, _, _, _, _) = setup_contract(&env);

    let config = client.get_config();
    assert_eq!(config.admin, admin);
    assert_eq!(config.oracle, oracle);
    assert_eq!(config.lock_period, 7 * 24 * 60 * 60);
    assert_eq!(config.staker_yield_share, 3000);
    assert_eq!(config.solver_yield_share, 7000);
    assert!(!config.paused);
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_double_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, oracle, staking_token_client, reward_token_client, _, _) =
        setup_contract(&env);

    // Try to initialize again
    client.initialize(
        &admin,
        &staking_token_client.address,
        &reward_token_client.address,
        &oracle,
        &604800u64,
    );
}

#[test]
fn test_stake() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, staker, _, staking_token_client, _, staking_admin_client, _) =
        setup_contract(&env);

    // Mint tokens to staker
    staking_admin_client.mint(&staker, &20_000_000_000); // 20,000 tokens

    // Stake tokens
    client.stake(&staker, &10_000_000_000); // 10,000 tokens

    // Verify staking
    let stake_info = client.get_stake(&staker);
    assert_eq!(stake_info.position.amount, 10_000_000_000);
    assert_eq!(stake_info.position.staked_at, 1000);

    // Verify total staked
    assert_eq!(client.get_total_staked(), 10_000_000_000);

    // Verify token transfer
    assert_eq!(staking_token_client.balance(&staker), 10_000_000_000);
}

#[test]
fn test_stake_multiple_times() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, staker, _, _, _, staking_admin_client, _) = setup_contract(&env);

    // Mint tokens
    staking_admin_client.mint(&staker, &200_000_000_000);

    // First stake
    client.stake(&staker, &50_000_000_000);
    let stake_info = client.get_stake(&staker);
    assert_eq!(stake_info.position.amount, 50_000_000_000);

    // Second stake
    env.ledger().set_timestamp(2000);
    client.stake(&staker, &60_000_000_000);

    let stake_info = client.get_stake(&staker);
    assert_eq!(stake_info.position.amount, 110_000_000_000);
    assert_eq!(stake_info.position.staked_at, 2000);
}

#[test]
fn test_unstake_after_lock_period() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, staker, _, staking_token_client, _, staking_admin_client, _) =
        setup_contract(&env);

    // Mint and stake tokens
    staking_admin_client.mint(&staker, &10_000_000_000);
    client.stake(&staker, &5_000_000_000);

    // Fast forward past lock period (7 days + 1 second)
    env.ledger().set_timestamp(1000 + 7 * 24 * 60 * 60 + 1);

    // Unstake
    client.unstake(&staker, &2_000_000_000);

    // Verify full amount returned (no penalty)
    assert_eq!(staking_token_client.balance(&staker), 7_000_000_000);

    // Verify staking info updated
    let stake_info = client.get_stake(&staker);
    assert_eq!(stake_info.position.amount, 3_000_000_000);
}

#[test]
#[should_panic(expected = "Stake is still locked")]
fn test_unstake_before_lock_period() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, staker, _, _, _, staking_admin_client, _) = setup_contract(&env);

    // Mint and stake tokens
    staking_admin_client.mint(&staker, &10_000_000_000);
    client.stake(&staker, &5_000_000_000);

    // Only advance 1 day (before 7-day lock)
    env.ledger().set_timestamp(1000 + 24 * 60 * 60);

    // Try to unstake (should fail)
    client.unstake(&staker, &1_000_000_000);
}

#[test]
fn test_record_solve() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, _, oracle, _, _, _, _) = setup_contract(&env);

    let player = Address::generate(&env);

    // Record solves with different difficulties
    client.record_solve(&oracle, &player, &5); // difficulty 5
    client.record_solve(&oracle, &player, &3); // difficulty 3

    // Verify player solves
    let epoch_id = client.get_current_epoch_id();
    let player_solves = client.get_player_solves(&player, &epoch_id).unwrap();
    assert_eq!(player_solves.weighted_solves, 8); // 5 + 3

    // Verify epoch total solves
    let epoch = client.get_epoch(&epoch_id);
    assert_eq!(epoch.total_solves, 8);
}

#[test]
#[should_panic(expected = "Oracle only")]
fn test_record_solve_non_oracle() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, staker, _, _, _, _, _) = setup_contract(&env);

    let player = Address::generate(&env);

    // Try to record solve as non-oracle
    client.record_solve(&staker, &player, &5);
}

#[test]
fn test_close_epoch() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(0);

    let (client, admin, _, _, _, reward_token_client, _, reward_admin_client) = setup_contract(&env);

    // Fast forward past epoch end (7 days + 1 second)
    env.ledger().set_timestamp(7 * 24 * 60 * 60 + 1);

    // Add rewards
    reward_admin_client.mint(&admin, &1_000_000_000_000);
    client.add_rewards(&admin, &1_000_000_000_000);

    // Close epoch with reward budget
    client.close_epoch(&admin, &1_000_000_000_000);

    // Verify epoch is closed
    let epoch = client.get_epoch(&0);
    assert!(epoch.distributed);
    assert_eq!(epoch.reward_budget, 1_000_000_000_000);

    // Verify new epoch created
    assert_eq!(client.get_current_epoch_id(), 1);
}

#[test]
#[should_panic(expected = "Epoch has not ended yet")]
fn test_close_epoch_too_early() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(0);

    let (client, admin, _, _, _, _, _, _) = setup_contract(&env);

    // Try to close before epoch ends
    env.ledger().set_timestamp(6 * 24 * 60 * 60);
    client.close_epoch(&admin, &1_000_000_000_000);
}

#[test]
fn test_claim_solver_reward() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(0);

    let (client, admin, _, oracle, _, reward_token_client, _, reward_admin_client) =
        setup_contract(&env);

    let player = Address::generate(&env);

    // Record solves
    client.record_solve(&oracle, &player, &10); // difficulty 10

    // Fast forward and close epoch
    env.ledger().set_timestamp(7 * 24 * 60 * 60 + 1);
    reward_admin_client.mint(&admin, &1_000_000_000_000);
    client.add_rewards(&admin, &1_000_000_000_000);
    client.close_epoch(&admin, &1_000_000_000_000);

    // Claim reward (70% of 1T = 700M, player has 100% of solves)
    let claimed = client.claim_epoch_reward(&player, &0);
    assert_eq!(claimed, 700_000_000_000);

    // Verify reward token balance
    assert_eq!(reward_token_client.balance(&player), 700_000_000_000);

    // Verify marked as claimed
    assert!(client.is_reward_claimed(&player, &0));
}

#[test]
fn test_claim_staker_reward() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(0);

    let (client, admin, staker, _, _, reward_token_client, staking_admin_client, reward_admin_client) =
        setup_contract(&env);

    // Stake tokens
    staking_admin_client.mint(&staker, &10_000_000_000);
    client.stake(&staker, &10_000_000_000);

    // Fast forward and close epoch
    env.ledger().set_timestamp(7 * 24 * 60 * 60 + 1);
    reward_admin_client.mint(&admin, &1_000_000_000_000);
    client.add_rewards(&admin, &1_000_000_000_000);
    client.close_epoch(&admin, &1_000_000_000_000);

    // Claim reward (30% of 1T = 300M, staker has 100% of stake)
    let claimed = client.claim_epoch_reward(&staker, &0);
    assert_eq!(claimed, 300_000_000_000);

    // Verify reward token balance
    assert_eq!(reward_token_client.balance(&staker), 300_000_000_000);
}

#[test]
fn test_claim_combined_reward() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(0);

    let (client, admin, staker, oracle, _, reward_token_client, staking_admin_client, reward_admin_client) =
        setup_contract(&env);

    // Stake tokens
    staking_admin_client.mint(&staker, &10_000_000_000);
    client.stake(&staker, &10_000_000_000);

    // Record solves
    client.record_solve(&oracle, &staker, &10);

    // Fast forward and close epoch
    env.ledger().set_timestamp(7 * 24 * 60 * 60 + 1);
    reward_admin_client.mint(&admin, &1_000_000_000_000);
    client.add_rewards(&admin, &1_000_000_000_000);
    client.close_epoch(&admin, &1_000_000_000_000);

    // Claim reward (30% staker + 70% solver = 100% since staker is also sole solver)
    let claimed = client.claim_epoch_reward(&staker, &0);
    assert_eq!(claimed, 1_000_000_000_000);
}

#[test]
#[should_panic(expected = "Reward already claimed for this epoch")]
fn test_claim_idempotent() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(0);

    let (client, admin, _, oracle, _, _, _, reward_admin_client) =
        setup_contract(&env);

    let player = Address::generate(&env);

    // Record solves
    client.record_solve(&oracle, &player, &10);

    // Fast forward and close epoch
    env.ledger().set_timestamp(7 * 24 * 60 * 60 + 1);
    reward_admin_client.mint(&admin, &1_000_000_000_000);
    client.add_rewards(&admin, &1_000_000_000_000);
    client.close_epoch(&admin, &1_000_000_000_000);

    // Claim reward
    client.claim_epoch_reward(&player, &0);

    // Try to claim again (should fail)
    client.claim_epoch_reward(&player, &0);
}

#[test]
fn test_proportional_reward_distribution() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(0);

    let (client, admin, _, oracle, _, _, _, reward_admin_client) =
        setup_contract(&env);

    let player1 = Address::generate(&env);
    let player2 = Address::generate(&env);

    // Record solves: player1 has 10, player2 has 5 (total 15)
    client.record_solve(&oracle, &player1, &10);
    client.record_solve(&oracle, &player2, &5);

    // Fast forward and close epoch
    env.ledger().set_timestamp(7 * 24 * 60 * 60 + 1);
    reward_admin_client.mint(&admin, &1_000_000_000_000);
    client.add_rewards(&admin, &1_000_000_000_000);
    client.close_epoch(&admin, &1_000_000_000_000);

    // Player1 should get 70% * (10/15) = 46.67% of 1T = 466.67B
    let claimed1 = client.claim_epoch_reward(&player1, &0);
    assert_eq!(claimed1, 466_666_666_666);

    // Player2 should get 70% * (5/15) = 23.33% of 1T = 233.33B (truncated)
    let claimed2 = client.claim_epoch_reward(&player2, &0);
    assert_eq!(claimed2, 233_333_333_333);
}

#[test]
fn test_get_stake_unclaimed_epochs() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(0);

    let (client, admin, staker, _, _, _, staking_admin_client, reward_admin_client) =
        setup_contract(&env);

    // Stake tokens
    staking_admin_client.mint(&staker, &10_000_000_000);
    client.stake(&staker, &10_000_000_000);

    // Close first epoch
    env.ledger().set_timestamp(7 * 24 * 60 * 60 + 1);
    reward_admin_client.mint(&admin, &1_000_000_000_000);
    client.add_rewards(&admin, &1_000_000_000_000);
    client.close_epoch(&admin, &1_000_000_000_000);

    // Check stake info
    let stake_info = client.get_stake(&staker);
    assert_eq!(stake_info.position.amount, 10_000_000_000);
    assert_eq!(stake_info.unclaimed_epochs.len(), 1);
    assert_eq!(stake_info.unclaimed_epochs.get(0).unwrap(), 0);
}

#[test]
fn test_pause_functionality() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, _, _, _, _) = setup_contract(&env);

    // Pause contract
    client.set_paused(&admin, &true);

    let config = client.get_config();
    assert!(config.paused);
}

#[test]
#[should_panic(expected = "Contract is paused")]
fn test_stake_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, staker, _, _, _, staking_admin_client, _) = setup_contract(&env);

    // Mint tokens
    staking_admin_client.mint(&staker, &10_000_000_000);

    // Pause contract
    client.set_paused(&admin, &true);

    // Try to stake (should fail)
    client.stake(&staker, &1_000_000_000);
}

#[test]
fn test_update_yield_shares() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, _, _, _, _) = setup_contract(&env);

    // Update yield shares
    client.update_yield_shares(&admin, &5000u32, &5000u32); // 50/50 split

    let config = client.get_config();
    assert_eq!(config.staker_yield_share, 5000);
    assert_eq!(config.solver_yield_share, 5000);
}

#[test]
#[should_panic(expected = "Yield shares must sum to 10000 basis points")]
fn test_invalid_yield_shares() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, _, _, _, _) = setup_contract(&env);

    // Invalid shares (don't sum to 10000)
    client.update_yield_shares(&admin, &6000u32, &6000u32);
}

#[test]
fn test_update_oracle() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, _, _, _, _) = setup_contract(&env);

    let new_oracle = Address::generate(&env);

    // Update oracle
    client.update_oracle(&admin, &new_oracle);

    let config = client.get_config();
    assert_eq!(config.oracle, new_oracle);
}

#[test]
#[should_panic(expected = "Admin only")]
fn test_admin_only_function() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, staker, _, _, _, _, _) = setup_contract(&env);

    // Try to update oracle as non-admin
    let new_oracle = Address::generate(&env);
    client.update_oracle(&staker, &new_oracle);
}

#[test]
fn test_stakers_list() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, _, _, _, _, staking_admin_client, _) = setup_contract(&env);

    let staker1 = Address::generate(&env);
    let staker2 = Address::generate(&env);
    let staker3 = Address::generate(&env);

    // Mint tokens
    staking_admin_client.mint(&staker1, &10_000_000_000);
    staking_admin_client.mint(&staker2, &10_000_000_000);
    staking_admin_client.mint(&staker3, &10_000_000_000);

    // Stake from multiple users
    client.stake(&staker1, &1_000_000_000);
    client.stake(&staker2, &2_000_000_000);
    client.stake(&staker3, &3_000_000_000);

    // Verify stakers list
    let stakers = client.get_all_stakers();
    assert_eq!(stakers.len(), 3);

    // Verify total staked
    assert_eq!(client.get_total_staked(), 6_000_000_000);

    // Full unstake one user
    env.ledger().set_timestamp(1000 + 7 * 24 * 60 * 60 + 1);
    client.unstake(&staker2, &2_000_000_000);

    // Staker2 should be removed from list
    let stakers = client.get_all_stakers();
    assert_eq!(stakers.len(), 2);
    assert!(!stakers.contains(&staker2));
}

#[test]
fn test_full_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(0);

    let (
        client,
        admin,
        staker,
        oracle,
        staking_token_client,
        reward_token_client,
        staking_admin_client,
        reward_admin_client,
    ) = setup_contract(&env);

    // 1. Stake tokens
    staking_admin_client.mint(&staker, &100_000_000_000);
    client.stake(&staker, &100_000_000_000);

    // 2. Record solves
    client.record_solve(&oracle, &staker, &20);

    // 3. Fast forward and close epoch
    env.ledger().set_timestamp(7 * 24 * 60 * 60 + 1);
    reward_admin_client.mint(&admin, &1_000_000_000_000);
    client.add_rewards(&admin, &1_000_000_000_000);
    client.close_epoch(&admin, &1_000_000_000_000);

    // 4. Claim rewards
    let claimed = client.claim_epoch_reward(&staker, &0);
    assert_eq!(claimed, 1_000_000_000_000);

    // 5. Unstake after lock period
    env.ledger().set_timestamp(7 * 24 * 60 * 60 + 1 + 7 * 24 * 60 * 60 + 1);
    client.unstake(&staker, &50_000_000_000);

    // Verify balances
    assert_eq!(staking_token_client.balance(&staker), 50_000_000_000);
    assert_eq!(reward_token_client.balance(&staker), 1_000_000_000_000);

    // Verify stake info
    let stake_info = client.get_stake(&staker);
    assert_eq!(stake_info.position.amount, 50_000_000_000);
}
