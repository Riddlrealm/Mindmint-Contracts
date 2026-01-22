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

fn setup_staking_contract(env: &Env) -> (
    StakingContractClient,
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
    let token_admin = Address::generate(env);

    // Create staking and reward tokens
    let (staking_token_addr, staking_token_client) = create_token_contract(env, &token_admin);
    let (reward_token_addr, reward_token_client) = create_token_contract(env, &token_admin);

    let staking_admin_client = StellarAssetClient::new(env, &staking_token_addr);
    let reward_admin_client = StellarAssetClient::new(env, &reward_token_addr);

    // Register staking contract
    let contract_id = env.register_contract(None, StakingContract);
    let client = StakingContractClient::new(env, &contract_id);

    // Initialize with 5% base APY (500 basis points) and 7 days lock period
    let base_apy = 500u32;
    let min_lock_period = 7 * 24 * 60 * 60u64; // 7 days

    client.initialize(
        &admin,
        &staking_token_addr,
        &reward_token_addr,
        &base_apy,
        &min_lock_period,
    );

    (
        client,
        admin,
        staker,
        token_admin,
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

    let (client, admin, _, _, _, _, _, _) = setup_staking_contract(&env);

    let config = client.get_config();
    assert_eq!(config.admin, admin);
    assert_eq!(config.base_apy, 500);
    assert_eq!(config.bronze_bonus, 100);
    assert_eq!(config.silver_bonus, 250);
    assert_eq!(config.gold_bonus, 500);
    assert!(!config.paused);
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_double_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, staking_token_client, reward_token_client, _, _) =
        setup_staking_contract(&env);

    // Try to initialize again
    client.initialize(
        &admin,
        &staking_token_client.address,
        &reward_token_client.address,
        &500u32,
        &604800u64,
    );
}

#[test]
fn test_stake() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, staker, _, staking_token_client, _, staking_admin_client, _) =
        setup_staking_contract(&env);

    // Mint tokens to staker
    staking_admin_client.mint(&staker, &20_000_000_000); // 20,000 tokens

    // Stake tokens
    client.stake(&staker, &10_000_000_000); // 10,000 tokens

    // Verify staking
    let staker_info = client.get_staker_info(&staker).unwrap();
    assert_eq!(staker_info.staked_amount, 10_000_000_000);
    assert_eq!(staker_info.stake_timestamp, 1000);
    assert_eq!(staker_info.tier, StakingTier::Silver); // 10,000 >= silver threshold

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

    let (client, _, staker, _, _, _, staking_admin_client, _) = setup_staking_contract(&env);

    // Mint tokens to staker
    staking_admin_client.mint(&staker, &200_000_000_000); // 200,000 tokens

    // First stake
    client.stake(&staker, &50_000_000_000); // 50,000 tokens
    let staker_info = client.get_staker_info(&staker).unwrap();
    assert_eq!(staker_info.tier, StakingTier::Silver);

    // Second stake to reach gold
    env.ledger().set_timestamp(2000);
    client.stake(&staker, &60_000_000_000); // 60,000 more tokens

    let staker_info = client.get_staker_info(&staker).unwrap();
    assert_eq!(staker_info.staked_amount, 110_000_000_000);
    assert_eq!(staker_info.tier, StakingTier::Gold); // 110,000 >= gold threshold
}

#[test]
fn test_tier_system() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, staker, _, _, _, staking_admin_client, _) = setup_staking_contract(&env);

    // Mint tokens
    staking_admin_client.mint(&staker, &500_000_000_000);

    // Test None tier (below bronze threshold)
    client.stake(&staker, &500_000_000); // 500 tokens
    let staker_info = client.get_staker_info(&staker).unwrap();
    assert_eq!(staker_info.tier, StakingTier::None);
    assert_eq!(client.get_current_apy(&staker), 500); // Base APY only

    // Stake more to reach Bronze
    client.stake(&staker, &600_000_000); // +600 = 1,100 tokens
    let staker_info = client.get_staker_info(&staker).unwrap();
    assert_eq!(staker_info.tier, StakingTier::Bronze);
    assert_eq!(client.get_current_apy(&staker), 600); // 500 + 100

    // Stake more to reach Silver
    client.stake(&staker, &9_000_000_000); // +9,000 = 10,100 tokens
    let staker_info = client.get_staker_info(&staker).unwrap();
    assert_eq!(staker_info.tier, StakingTier::Silver);
    assert_eq!(client.get_current_apy(&staker), 750); // 500 + 250

    // Stake more to reach Gold
    client.stake(&staker, &100_000_000_000); // +100,000 = 110,100 tokens
    let staker_info = client.get_staker_info(&staker).unwrap();
    assert_eq!(staker_info.tier, StakingTier::Gold);
    assert_eq!(client.get_current_apy(&staker), 1000); // 500 + 500
}

#[test]
fn test_unstake_after_lock_period() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, staker, _, staking_token_client, _, staking_admin_client, _) =
        setup_staking_contract(&env);

    // Mint and stake tokens
    staking_admin_client.mint(&staker, &10_000_000_000);
    client.stake(&staker, &5_000_000_000);

    // Fast forward past lock period (7 days + 1 second)
    env.ledger().set_timestamp(1000 + 7 * 24 * 60 * 60 + 1);

    // Verify can unstake without penalty
    assert!(client.can_unstake_without_penalty(&staker));
    assert_eq!(client.get_time_until_unlock(&staker), 0);

    // Unstake
    client.unstake(&staker, &2_000_000_000);

    // Verify no penalty applied (full amount returned)
    assert_eq!(staking_token_client.balance(&staker), 7_000_000_000); // 5B original + 2B unstaked = 7B

    // Verify staking info updated
    let staker_info = client.get_staker_info(&staker).unwrap();
    assert_eq!(staker_info.staked_amount, 3_000_000_000);
}

#[test]
fn test_early_unstake_penalty() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, staker, _, staking_token_client, _, staking_admin_client, _) =
        setup_staking_contract(&env);

    // Mint and stake tokens
    staking_admin_client.mint(&staker, &10_000_000_000);
    client.stake(&staker, &5_000_000_000);

    // Only advance 1 day (before 7-day lock)
    env.ledger().set_timestamp(1000 + 24 * 60 * 60);

    // Verify cannot unstake without penalty
    assert!(!client.can_unstake_without_penalty(&staker));
    assert!(client.get_time_until_unlock(&staker) > 0);

    // Unstake with penalty
    client.unstake(&staker, &1_000_000_000); // Unstake 1,000 tokens

    // 10% penalty = 100M tokens lost
    // Should receive: 1,000 - 100 = 900 tokens
    // Balance should be: 5B (original) + 900M (unstaked) = 5.9B
    assert_eq!(staking_token_client.balance(&staker), 5_900_000_000);
}

#[test]
fn test_rewards_calculation_and_claim() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(0);

    let (client, admin, staker, _, _, reward_token_client, staking_admin_client, reward_admin_client) =
        setup_staking_contract(&env);

    // Mint staking tokens and stake
    staking_admin_client.mint(&staker, &10_000_000_000);
    client.stake(&staker, &10_000_000_000); // Silver tier (10,000 tokens)

    // Add rewards to pool
    reward_admin_client.mint(&admin, &1_000_000_000_000);
    client.add_rewards(&admin, &1_000_000_000_000);

    // Fast forward 1 year
    env.ledger().set_timestamp(31_536_000);

    // Calculate expected rewards
    // Silver tier APY = 500 + 250 = 750 basis points = 7.5%
    // Expected: 10,000,000,000 * 750 / 10000 = 750,000,000 (750 tokens)
    let pending = client.get_pending_rewards(&staker);
    assert_eq!(pending, 750_000_000); // 750 tokens

    // Claim rewards
    let claimed = client.claim_rewards(&staker);
    assert_eq!(claimed, 750_000_000);

    // Verify reward token balance
    assert_eq!(reward_token_client.balance(&staker), 750_000_000);

    // Verify reward pool decreased
    assert_eq!(client.get_reward_pool(), 1_000_000_000_000 - 750_000_000);
}

#[test]
fn test_emergency_withdraw() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, staker, _, staking_token_client, _, staking_admin_client, _) =
        setup_staking_contract(&env);

    // Mint and stake tokens
    staking_admin_client.mint(&staker, &10_000_000_000);
    client.stake(&staker, &10_000_000_000);

    // Initial balance after staking should be 0
    assert_eq!(staking_token_client.balance(&staker), 0);

    // Emergency withdraw (20% penalty)
    let returned = client.emergency_withdraw(&staker);

    // 20% penalty = 2B tokens lost
    // Should receive: 10B - 2B = 8B
    assert_eq!(returned, 8_000_000_000);
    assert_eq!(staking_token_client.balance(&staker), 8_000_000_000);

    // Verify staker info cleared
    let staker_info = client.get_staker_info(&staker).unwrap();
    assert_eq!(staker_info.staked_amount, 0);
    assert_eq!(staker_info.tier, StakingTier::None);

    // Verify total staked is 0
    assert_eq!(client.get_total_staked(), 0);
}

#[test]
fn test_pause_functionality() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, admin, staker, _, _, _, staking_admin_client, _) = setup_staking_contract(&env);

    // Mint tokens
    staking_admin_client.mint(&staker, &10_000_000_000);

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
    env.ledger().set_timestamp(1000);

    let (client, admin, staker, _, _, _, staking_admin_client, _) = setup_staking_contract(&env);

    // Mint tokens
    staking_admin_client.mint(&staker, &10_000_000_000);

    // Pause contract
    client.set_paused(&admin, &true);

    // Try to stake (should fail)
    client.stake(&staker, &1_000_000_000);
}

#[test]
fn test_emergency_withdraw_when_paused() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, admin, staker, _, staking_token_client, _, staking_admin_client, _) =
        setup_staking_contract(&env);

    // Mint and stake tokens
    staking_admin_client.mint(&staker, &10_000_000_000);
    client.stake(&staker, &10_000_000_000);

    // Pause contract
    client.set_paused(&admin, &true);

    // Emergency withdraw should still work (no pause check)
    let returned = client.emergency_withdraw(&staker);
    assert_eq!(returned, 8_000_000_000);
    assert_eq!(staking_token_client.balance(&staker), 8_000_000_000);
}

#[test]
fn test_update_apy_config() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, _, _, _, _) = setup_staking_contract(&env);

    // Update APY config
    client.update_apy_config(
        &admin,
        &1000u32, // 10% base APY
        &200u32,  // 2% bronze bonus
        &400u32,  // 4% silver bonus
        &800u32,  // 8% gold bonus
    );

    let config = client.get_config();
    assert_eq!(config.base_apy, 1000);
    assert_eq!(config.bronze_bonus, 200);
    assert_eq!(config.silver_bonus, 400);
    assert_eq!(config.gold_bonus, 800);
}

#[test]
fn test_update_tier_thresholds() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, _, _, _, _) = setup_staking_contract(&env);

    // Update tier thresholds
    client.update_tier_thresholds(
        &admin,
        &2_000_000_000,   // 2,000 bronze threshold
        &20_000_000_000,  // 20,000 silver threshold
        &200_000_000_000, // 200,000 gold threshold
    );

    let config = client.get_config();
    assert_eq!(config.bronze_threshold, 2_000_000_000);
    assert_eq!(config.silver_threshold, 20_000_000_000);
    assert_eq!(config.gold_threshold, 200_000_000_000);
}

#[test]
fn test_update_staking_params() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, _, _, _, _) = setup_staking_contract(&env);

    // Update staking params
    client.update_staking_params(
        &admin,
        &(14 * 24 * 60 * 60), // 14 days lock
        &1500u32,             // 15% early penalty
        &3000u32,             // 30% emergency penalty
    );

    let config = client.get_config();
    assert_eq!(config.min_lock_period, 14 * 24 * 60 * 60);
    assert_eq!(config.early_unstake_penalty, 1500);
    assert_eq!(config.emergency_penalty, 3000);
}

#[test]
#[should_panic(expected = "Admin only")]
fn test_update_config_non_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, staker, _, _, _, _, _) = setup_staking_contract(&env);

    // Try to update APY config as non-admin
    client.update_apy_config(
        &staker,
        &1000u32,
        &200u32,
        &400u32,
        &800u32,
    );
}

#[test]
#[should_panic(expected = "Amount must be positive")]
fn test_stake_zero_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, staker, _, _, _, _, _) = setup_staking_contract(&env);

    client.stake(&staker, &0);
}

#[test]
#[should_panic(expected = "Insufficient staked balance")]
fn test_unstake_more_than_staked() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, staker, _, _, _, staking_admin_client, _) = setup_staking_contract(&env);

    staking_admin_client.mint(&staker, &10_000_000_000);
    client.stake(&staker, &5_000_000_000);

    // Try to unstake more than staked
    client.unstake(&staker, &6_000_000_000);
}

#[test]
#[should_panic(expected = "No rewards to claim")]
fn test_claim_zero_rewards() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, staker, _, _, _, staking_admin_client, _) = setup_staking_contract(&env);

    staking_admin_client.mint(&staker, &10_000_000_000);
    client.stake(&staker, &5_000_000_000);

    // Try to claim immediately (no time passed, no rewards)
    client.claim_rewards(&staker);
}

#[test]
#[should_panic(expected = "Insufficient reward pool")]
fn test_claim_rewards_insufficient_pool() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(0);

    let (client, _, staker, _, _, _, staking_admin_client, _) = setup_staking_contract(&env);

    // Stake a smaller amount to avoid overflow and ensure rewards calculation works
    staking_admin_client.mint(&staker, &1_000_000_000);
    client.stake(&staker, &1_000_000_000); // 1,000 tokens

    // Don't add rewards to pool

    // Fast forward 30 days
    env.ledger().set_timestamp(30 * 24 * 60 * 60);

    // Try to claim (should fail - no rewards in pool)
    client.claim_rewards(&staker);
}

#[test]
fn test_stakers_list() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, staker, _, _, _, staking_admin_client, _) = setup_staking_contract(&env);

    let staker2 = Address::generate(&env);
    let staker3 = Address::generate(&env);

    // Mint tokens
    staking_admin_client.mint(&staker, &10_000_000_000);
    staking_admin_client.mint(&staker2, &10_000_000_000);
    staking_admin_client.mint(&staker3, &10_000_000_000);

    // Stake from multiple users
    client.stake(&staker, &1_000_000_000);
    client.stake(&staker2, &2_000_000_000);
    client.stake(&staker3, &3_000_000_000);

    // Verify stakers list
    let stakers = client.get_all_stakers();
    assert_eq!(stakers.len(), 3);

    // Verify total staked
    assert_eq!(client.get_total_staked(), 6_000_000_000);

    // Full unstake one user
    env.ledger().set_timestamp(1000 + 7 * 24 * 60 * 60 + 1); // Past lock period
    client.unstake(&staker2, &2_000_000_000);

    // Staker2 should be removed from list
    let stakers = client.get_all_stakers();
    assert_eq!(stakers.len(), 2);
    assert!(!stakers.contains(&staker2));
}

#[test]
fn test_rewards_accumulation_on_additional_stake() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(0);

    let (client, admin, staker, _, _, _, staking_admin_client, reward_admin_client) =
        setup_staking_contract(&env);

    // Mint tokens
    staking_admin_client.mint(&staker, &50_000_000_000);
    reward_admin_client.mint(&admin, &1_000_000_000_000);
    client.add_rewards(&admin, &1_000_000_000_000);

    // First stake - Silver tier (10,000 tokens)
    client.stake(&staker, &10_000_000_000);

    // Fast forward 6 months
    env.ledger().set_timestamp(31_536_000 / 2);

    // Get pending rewards before additional stake
    // Silver tier APY = 750 basis points = 7.5%
    // 6 months: 10,000 * 0.075 * 0.5 = 375 tokens
    let pending_before = client.get_pending_rewards(&staker);
    assert!(pending_before > 0);

    // Additional stake (this should accumulate pending rewards)
    client.stake(&staker, &10_000_000_000); // 10,000 more tokens

    // Verify accumulated rewards are preserved in staker info
    let staker_info = client.get_staker_info(&staker).unwrap();
    assert_eq!(staker_info.accumulated_rewards, pending_before);
    assert_eq!(staker_info.staked_amount, 20_000_000_000);
}

#[test]
fn test_full_staking_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(0);

    let (
        client,
        admin,
        staker,
        _,
        staking_token_client,
        reward_token_client,
        staking_admin_client,
        reward_admin_client,
    ) = setup_staking_contract(&env);

    // 1. Setup - mint tokens
    staking_admin_client.mint(&staker, &100_000_000_000); // 100,000 staking tokens
    reward_admin_client.mint(&admin, &10_000_000_000_000); // 10M reward tokens
    client.add_rewards(&admin, &10_000_000_000_000);

    // 2. Initial stake to reach Gold tier
    client.stake(&staker, &100_000_000_000); // 100,000 tokens

    // Verify Gold tier
    let staker_info = client.get_staker_info(&staker).unwrap();
    assert_eq!(staker_info.tier, StakingTier::Gold);
    assert_eq!(client.get_current_apy(&staker), 1000); // 10% APY (500 base + 500 gold bonus)

    // 3. Fast forward 30 days (past the 7-day lock period)
    let thirty_days = 30 * 24 * 60 * 60u64;
    env.ledger().set_timestamp(thirty_days);

    // 4. Check pending rewards
    let pending = client.get_pending_rewards(&staker);
    // Gold tier APY = 1000 basis points = 10%
    // Expected: 100,000,000,000 * 1000 / 10000 * (30 days / 365 days)
    // = 10,000,000,000 * 30 / 365 = ~821,917,808
    assert!(pending > 800_000_000); // Approximately 821 tokens

    // 5. Claim rewards
    let claimed = client.claim_rewards(&staker);
    assert_eq!(claimed, pending);
    assert_eq!(reward_token_client.balance(&staker), claimed);

    // 6. Partial unstake (no penalty after lock period)
    client.unstake(&staker, &50_000_000_000); // Unstake 50,000

    // Verify tier downgrade
    let staker_info = client.get_staker_info(&staker).unwrap();
    assert_eq!(staker_info.staked_amount, 50_000_000_000);
    assert_eq!(staker_info.tier, StakingTier::Silver); // Dropped below Gold threshold

    // 7. Fast forward another 30 days
    env.ledger().set_timestamp(thirty_days * 2);

    // 8. Emergency withdraw remaining
    let emergency_amount = client.emergency_withdraw(&staker);

    // 20% penalty on 50,000 = 10,000 tokens penalty
    assert_eq!(emergency_amount, 40_000_000_000);

    // Final balances
    // Staking tokens: 50,000 (unstaked) + 40,000 (emergency) = 90,000
    assert_eq!(staking_token_client.balance(&staker), 90_000_000_000);

    // Verify staker cleared
    let staker_info = client.get_staker_info(&staker).unwrap();
    assert_eq!(staker_info.staked_amount, 0);
    assert_eq!(client.get_total_staked(), 0);
}
