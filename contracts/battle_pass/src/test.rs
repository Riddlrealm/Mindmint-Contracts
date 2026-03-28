#![cfg(test)]

use crate::{BattlePassContract, BattlePassContractClient, BattlePass, PassTier, Season, RewardType};
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_contract_initialization() {
    let env = Env::default();
    let contract_id = env.register_contract(None, BattlePassContract);
    let client = BattlePassContractClient::new(&env, &contract_id);

    let admin = Address::random(&env);
    
    // Initialize contract
    client.init(&admin);

    // Verify admin is set
    // Note: In a real test, you'd add a get_admin function to verify
}

#[test]
fn test_create_season() {
    let env = Env::default();
    let contract_id = env.register_contract(None, BattlePassContract);
    let client = BattlePassContractClient::new(&env, &contract_id);

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    
    // Initialize
    client.init(&admin);
    
    // Create season
    let now = env.ledger().timestamp();
    client.create_season(
        &1,
        &now,
        &(now + 2_592_000), // 30 days later
        &1000u128,
        &oracle,
    );
}

#[test]
fn test_configure_season_tiers() {
    let env = Env::default();
    let contract_id = env.register_contract(None, BattlePassContract);
    let client = BattlePassContractClient::new(&env, &contract_id);

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    
    // Initialize and create season
    client.init(&admin);
    
    let now = env.ledger().timestamp();
    client.create_season(
        &1,
        &(now + 1000), // Start in future
        &(now + 2_592_000 + 1000),
        &1000u128,
        &oracle,
    );
    
    // Configure tiers
    let mut tiers = Vec::new(&env);
    tiers.push_back(PassTier {
        required_xp: 1000,
        reward_type: RewardType::Token,
        reward_amount: 100u128,
    });
    tiers.push_back(PassTier {
        required_xp: 2500,
        reward_type: RewardType::Cosmetic,
        reward_amount: 1u128,
    });
    tiers.push_back(PassTier {
        required_xp: 5000,
        reward_type: RewardType::Nft,
        reward_amount: 1u128,
    });
    
    client.configure_season_tiers(&1, &tiers);
}

#[test]
#[should_panic(expected = "Cannot configure after season start")]
fn test_cannot_configure_after_start() {
    let env = Env::default();
    let contract_id = env.register_contract(None, BattlePassContract);
    let client = BattlePassContractClient::new(&env, &contract_id);

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    
    // Initialize and create season starting now
    client.init(&admin);
    
    let now = env.ledger().timestamp();
    client.create_season(
        &1,
        &now,
        &(now + 2_592_000),
        &1000u128,
        &oracle,
    );
    
    // Try to configure after start - should fail
    let tiers = Vec::new(&env);
    client.configure_season_tiers(&1, &tiers);
}

#[test]
fn test_purchase_pass() {
    let env = Env::default();
    let contract_id = env.register_contract(None, BattlePassContract);
    let client = BattlePassContractClient::new(&env, &contract_id);

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    let player = Address::random(&env);
    
    // Setup season
    client.init(&admin);
    
    let now = env.ledger().timestamp();
    client.create_season(&1, &now, &(now + 2_592_000), &1000u128, &oracle);
    
    // Configure and activate season
    let mut tiers = Vec::new(&env);
    tiers.push_back(PassTier {
        required_xp: 1000,
        reward_type: RewardType::Token,
        reward_amount: 100u128,
    });
    client.configure_season_tiers(&1, &tiers);
    client.activate_season(&1);
    
    // Purchase pass
    client.purchase_pass(&player, &1);
    
    // TODO: Verify pass exists - would need get_pass_by_player function
}

#[test]
#[should_panic(expected = "Season is not active")]
fn test_cannot_purchase_outside_window() {
    let env = Env::default();
    let contract_id = env.register_contract(None, BattlePassContract);
    let client = BattlePassContractClient::new(&env, &contract_id);

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    let player = Address::random(&env);
    
    // Setup season in future
    client.init(&admin);
    
    let now = env.ledger().timestamp();
    client.create_season(
        &1,
        &(now + 1000), // Start in future
        &(now + 2_592_000 + 1000),
        &1000u128,
        &oracle,
    );
    
    client.activate_season(&1);
    
    // Try to purchase before season starts - should fail
    client.purchase_pass(&player, &1);
}

#[test]
fn test_add_xp_and_tier_advancement() {
    let env = Env::default();
    let contract_id = env.register_contract(None, BattlePassContract);
    let client = BattlePassContractClient::new(&env, &contract_id);

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    let player = Address::random(&env);
    
    // Setup season with tiers
    client.init(&admin);
    
    let now = env.ledger().timestamp();
    client.create_season(&1, &now, &(now + 2_592_000), &1000u128, &oracle);
    
    let mut tiers = Vec::new(&env);
    tiers.push_back(PassTier {
        required_xp: 1000,
        reward_type: RewardType::Token,
        reward_amount: 100u128,
    });
    tiers.push_back(PassTier {
        required_xp: 2500,
        reward_type: RewardType::Token,
        reward_amount: 200u128,
    });
    
    client.configure_season_tiers(&1, &tiers);
    client.activate_season(&1);
    client.purchase_pass(&player, &1);
    
    // Get pass ID - would need get_pass_by_player function
    // For now, assume pass ID is 1
    let pass_id = 1u32;
    
    // Add XP as oracle
    client.add_xp(&pass_id, &1500);
    
    // Check tier advancement
    let (xp, tier, claimed) = client.get_pass(&pass_id);
    assert_eq!(xp, 1500);
    assert_eq!(tier, 1); // Should have reached tier 1 (1000 XP)
    assert_eq!(claimed.len(), 0);
}

#[test]
fn test_claim_tier_reward() {
    let env = Env::default();
    let contract_id = env.register_contract(None, BattlePassContract);
    let client = BattlePassContractClient::new(&env, &contract_id);

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    let player = Address::random(&env);
    
    // Setup season
    client.init(&admin);
    
    let now = env.ledger().timestamp();
    client.create_season(&1, &now, &(now + 2_592_000), &1000u128, &oracle);
    
    let mut tiers = Vec::new(&env);
    tiers.push_back(PassTier {
        required_xp: 1000,
        reward_type: RewardType::Token,
        reward_amount: 100u128,
    });
    
    client.configure_season_tiers(&1, &tiers);
    client.activate_season(&1);
    client.purchase_pass(&player, &1);
    
    let pass_id = 1u32;
    
    // Add XP to reach tier
    client.add_xp(&pass_id, &1500);
    
    // Claim reward
    let reward = client.claim_tier_reward(&pass_id, &0);
    assert_eq!(reward.reward_amount, 100u128);
    assert!(matches!(reward.reward_type, RewardType::Token));
    
    // Verify reward is marked as claimed
    let (_, _, claimed) = client.get_pass(&pass_id);
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed.get(0).unwrap(), &0);
}

#[test]
#[should_panic(expected = "Reward already claimed")]
fn test_cannot_claim_same_reward_twice() {
    let env = Env::default();
    let contract_id = env.register_contract(None, BattlePassContract);
    let client = BattlePassContractClient::new(&env, &contract_id);

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    let player = Address::random(&env);
    
    // Setup season
    client.init(&admin);
    
    let now = env.ledger().timestamp();
    client.create_season(&1, &now, &(now + 2_592_000), &1000u128, &oracle);
    
    let mut tiers = Vec::new(&env);
    tiers.push_back(PassTier {
        required_xp: 1000,
        reward_type: RewardType::Token,
        reward_amount: 100u128,
    });
    
    client.configure_season_tiers(&1, &tiers);
    client.activate_season(&1);
    client.purchase_pass(&player, &1);
    
    let pass_id = 1u32;
    
    // Add XP and claim
    client.add_xp(&pass_id, &1500);
    client.claim_tier_reward(&pass_id, &0);
    
    // Try to claim again - should fail
    client.claim_tier_reward(&pass_id, &0);
}

#[test]
#[should_panic(expected = "Tier not reached yet")]
fn test_cannot_claim_unreached_tier() {
    let env = Env::default();
    let contract_id = env.register_contract(None, BattlePassContract);
    let client = BattlePassContractClient::new(&env, &contract_id);

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    let player = Address::random(&env);
    
    // Setup season
    client.init(&admin);
    
    let now = env.ledger().timestamp();
    client.create_season(&1, &now, &(now + 2_592_000), &1000u128, &oracle);
    
    let mut tiers = Vec::new(&env);
    tiers.push_back(PassTier {
        required_xp: 1000,
        reward_type: RewardType::Token,
        reward_amount: 100u128,
    });
    tiers.push_back(PassTier {
        required_xp: 2500,
        reward_type: RewardType::Token,
        reward_amount: 200u128,
    });
    
    client.configure_season_tiers(&1, &tiers);
    client.activate_season(&1);
    client.purchase_pass(&player, &1);
    
    let pass_id = 1u32;
    
    // Add XP for only tier 0
    client.add_xp(&pass_id, &1500);
    
    // Try to claim tier 1 - should fail
    client.claim_tier_reward(&pass_id, &1);
}

#[test]
#[should_panic(expected = "Season is not active")]
fn test_cannot_add_xp_expired_season() {
    let env = Env::default();
    let contract_id = env.register_contract(None, BattlePassContract);
    let client = BattlePassContractClient::new(&env, &contract_id);

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    let player = Address::random(&env);
    
    // Setup expired season
    client.init(&admin);
    
    let now = env.ledger().timestamp();
    client.create_season(
        &1,
        &(now - 2_592_000), // Started 30 days ago
        &(now - 1000),     // Ended 1000 seconds ago
        &1000u128,
        &oracle,
    );
    
    client.activate_season(&1);
    client.purchase_pass(&player, &1);
    
    let pass_id = 1u32;
    
    // Try to add XP to expired season - should fail
    client.add_xp(&pass_id, &1000);
}

#[test]
fn test_multiple_reward_types() {
    let env = Env::default();
    let contract_id = env.register_contract(None, BattlePassContract);
    let client = BattlePassContractClient::new(&env, &contract_id);

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    let player = Address::random(&env);
    
    // Setup season with different reward types
    client.init(&admin);
    
    let now = env.ledger().timestamp();
    client.create_season(&1, &now, &(now + 2_592_000), &1000u128, &oracle);
    
    let mut tiers = Vec::new(&env);
    tiers.push_back(PassTier {
        required_xp: 1000,
        reward_type: RewardType::Token,
        reward_amount: 100u128,
    });
    tiers.push_back(PassTier {
        required_xp: 2500,
        reward_type: RewardType::Cosmetic,
        reward_amount: 1u128,
    });
    tiers.push_back(PassTier {
        required_xp: 5000,
        reward_type: RewardType::Nft,
        reward_amount: 1u128,
    });
    
    client.configure_season_tiers(&1, &tiers);
    client.activate_season(&1);
    client.purchase_pass(&player, &1);
    
    let pass_id = 1u32;
    
    // Add enough XP for all tiers
    client.add_xp(&pass_id, &6000);
    
    // Claim different reward types
    let token_reward = client.claim_tier_reward(&pass_id, &0);
    assert!(matches!(token_reward.reward_type, RewardType::Token));
    
    let cosmetic_reward = client.claim_tier_reward(&pass_id, &1);
    assert!(matches!(cosmetic_reward.reward_type, RewardType::Cosmetic));
    
    let nft_reward = client.claim_tier_reward(&pass_id, &2);
    assert!(matches!(nft_reward.reward_type, RewardType::Nft));
}

#[test]
fn test_tier_advancement_events() {
    let env = Env::default();
    let contract_id = env.register_contract(None, BattlePassContract);
    let client = BattlePassContractClient::new(&env, &contract_id);

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    let player = Address::random(&env);
    
    // Setup season
    client.init(&admin);
    
    let now = env.ledger().timestamp();
    client.create_season(&1, &now, &(now + 2_592_000), &1000u128, &oracle);
    
    let mut tiers = Vec::new(&env);
    tiers.push_back(PassTier {
        required_xp: 1000,
        reward_type: RewardType::Token,
        reward_amount: 100u128,
    });
    tiers.push_back(PassTier {
        required_xp: 2500,
        reward_type: RewardType::Token,
        reward_amount: 200u128,
    });
    
    client.configure_season_tiers(&1, &tiers);
    client.activate_season(&1);
    client.purchase_pass(&player, &1);
    
    let pass_id = 1u32;
    
    // Add XP to advance multiple tiers
    client.add_xp(&pass_id, &3000);
    
    // TODO: Verify TierReached events were emitted
    // In a real test, you'd check the event logs
}
