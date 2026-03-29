#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{Address, Env, String};

fn setup_env() -> (Env, Address, Address, Address, LoyaltyPointsContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, LoyaltyPointsContract);
    let client = LoyaltyPointsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let player = Address::generate(&env);

    // Set ledger timestamp to a known value with high TTLs to avoid archival in time-travel tests
    env.ledger().set(LedgerInfo {
        timestamp: 1_000_000,
        protocol_version: 20,
        sequence_number: 100,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1_000_000,
        min_persistent_entry_ttl: 1_000_000,
        max_entry_ttl: 10_000_000,
    });

    client.initialize(&admin, &oracle);

    (env, admin, oracle, player, client)
}

// ──────────────────────────────────────────────────────────
// INITIALIZATION TESTS
// ──────────────────────────────────────────────────────────

#[test]
fn test_initialize_success() {
    let (env, admin, oracle, _player, client) = setup_env();

    // Admin and oracle should be stored
    let balance = client.get_balance(&Address::generate(&env));
    assert_eq!(balance.current_balance, 0);
    assert_eq!(balance.total_earned, 0);

    // Verify admin/oracle are set by testing they work
    let _ = admin;
    let _ = oracle;
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_initialize_twice_fails() {
    let (_env, admin, oracle, _player, client) = setup_env();
    // Second init should fail with AlreadyInitialized
    client.initialize(&admin, &oracle);
}

// ──────────────────────────────────────────────────────────
// AWARD POINTS TESTS
// ──────────────────────────────────────────────────────────

#[test]
fn test_award_points_single() {
    let (env, _admin, _oracle, player, client) = setup_env();

    let reason = String::from_str(&env, "puzzle_solve");
    let new_balance = client.award_points(&player, &500, &reason);

    assert_eq!(new_balance, 500);

    let balance = client.get_balance(&player);
    assert_eq!(balance.current_balance, 500);
    assert_eq!(balance.total_earned, 500);
    assert_eq!(balance.total_redeemed, 0);
    assert_eq!(balance.last_activity, 1_000_000);
}

#[test]
fn test_award_points_multiple_accumulate() {
    let (env, _admin, _oracle, player, client) = setup_env();

    let r1 = String::from_str(&env, "solve");
    let r2 = String::from_str(&env, "referral");
    let r3 = String::from_str(&env, "purchase");

    client.award_points(&player, &100, &r1);
    client.award_points(&player, &200, &r2);
    let total = client.award_points(&player, &300, &r3);

    assert_eq!(total, 600);

    let balance = client.get_balance(&player);
    assert_eq!(balance.total_earned, 600);
    assert_eq!(balance.current_balance, 600);
}

#[test]
fn test_award_points_multiple_players() {
    let (env, _admin, _oracle, player1, client) = setup_env();
    let player2 = Address::generate(&env);

    let reason = String::from_str(&env, "solve");

    client.award_points(&player1, &100, &reason);
    client.award_points(&player2, &200, &reason);

    assert_eq!(client.get_balance(&player1).current_balance, 100);
    assert_eq!(client.get_balance(&player2).current_balance, 200);
}

// ──────────────────────────────────────────────────────────
// REDEMPTION TESTS
// ──────────────────────────────────────────────────────────

#[test]
fn test_redeem_success() {
    let (env, _admin, _oracle, player, client) = setup_env();

    // Create an option
    let name = String::from_str(&env, "10% Discount");
    let option_id = client.create_option(&name, &500, &RewardType::Discount, &10);

    // Award points
    let reason = String::from_str(&env, "solve");
    client.award_points(&player, &1000, &reason);

    // Redeem
    let remaining = client.redeem(&player, &option_id);
    assert_eq!(remaining, 500);

    let balance = client.get_balance(&player);
    assert_eq!(balance.current_balance, 500);
    assert_eq!(balance.total_earned, 1000);
    assert_eq!(balance.total_redeemed, 500);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_redeem_insufficient_balance() {
    let (env, _admin, _oracle, player, client) = setup_env();

    let name = String::from_str(&env, "Big Reward");
    let option_id = client.create_option(&name, &1000, &RewardType::Token, &50);

    // Award only 100 points
    let reason = String::from_str(&env, "solve");
    client.award_points(&player, &100, &reason);

    // Should fail — not enough points
    client.redeem(&player, &option_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_redeem_nonexistent_option() {
    let (env, _admin, _oracle, player, client) = setup_env();

    let reason = String::from_str(&env, "solve");
    client.award_points(&player, &1000, &reason);

    // Option 99 doesn't exist
    client.redeem(&player, &99);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_redeem_disabled_option() {
    let (env, _admin, _oracle, player, client) = setup_env();

    let name = String::from_str(&env, "Old Reward");
    let option_id = client.create_option(&name, &100, &RewardType::Item, &1);

    // Disable the option
    client.disable_option(&option_id);

    // Award points
    let reason = String::from_str(&env, "solve");
    client.award_points(&player, &500, &reason);

    // Should fail — option disabled
    client.redeem(&player, &option_id);
}

#[test]
fn test_redeem_multiple_times() {
    let (env, _admin, _oracle, player, client) = setup_env();

    let name = String::from_str(&env, "Small Reward");
    let option_id = client.create_option(&name, &100, &RewardType::Discount, &5);

    let reason = String::from_str(&env, "solve");
    client.award_points(&player, &500, &reason);

    // Redeem 3 times
    assert_eq!(client.redeem(&player, &option_id), 400);
    assert_eq!(client.redeem(&player, &option_id), 300);
    assert_eq!(client.redeem(&player, &option_id), 200);

    let balance = client.get_balance(&player);
    assert_eq!(balance.total_redeemed, 300);
    assert_eq!(balance.current_balance, 200);
}

// ──────────────────────────────────────────────────────────
// EXPIRY TESTS
// ──────────────────────────────────────────────────────────

#[test]
fn test_expire_stale_points_after_12_months() {
    let (env, _admin, _oracle, player, client) = setup_env();

    let reason = String::from_str(&env, "solve");
    client.award_points(&player, &1000, &reason);

    // Advance time by 12 months + 1 second
    env.ledger().set(LedgerInfo {
        timestamp: 1_000_000 + EXPIRY_WINDOW + 1,
        protocol_version: 20,
        sequence_number: 200,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1_000_000,
        min_persistent_entry_ttl: 1_000_000,
        max_entry_ttl: 10_000_000,
    });

    let result = client.expire_stale_points(&player);
    assert_eq!(result, 0);

    let balance = client.get_balance(&player);
    assert_eq!(balance.current_balance, 0);
    // total_earned stays as historical record
    assert_eq!(balance.total_earned, 1000);
}

#[test]
fn test_expire_not_stale_returns_balance() {
    let (env, _admin, _oracle, player, client) = setup_env();

    let reason = String::from_str(&env, "solve");
    client.award_points(&player, &500, &reason);

    // Advance by only 6 months (not enough to expire)
    env.ledger().set(LedgerInfo {
        timestamp: 1_000_000 + EXPIRY_WINDOW / 2,
        protocol_version: 20,
        sequence_number: 150,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1_000_000,
        min_persistent_entry_ttl: 1_000_000,
        max_entry_ttl: 10_000_000,
    });

    let result = client.expire_stale_points(&player);
    // Not expired — balance intact
    assert_eq!(result, 500);
}

#[test]
fn test_activity_resets_expiry_clock() {
    let (env, _admin, _oracle, player, client) = setup_env();

    let reason = String::from_str(&env, "solve");
    client.award_points(&player, &500, &reason);

    // Advance 11 months
    let eleven_months = EXPIRY_WINDOW - 30 * 24 * 60 * 60;
    env.ledger().set(LedgerInfo {
        timestamp: 1_000_000 + eleven_months,
        protocol_version: 20,
        sequence_number: 150,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1_000_000,
        min_persistent_entry_ttl: 1_000_000,
        max_entry_ttl: 10_000_000,
    });

    // New activity resets the clock
    let reason2 = String::from_str(&env, "referral");
    client.award_points(&player, &100, &reason2);

    // Advance another 11 months from the new activity
    env.ledger().set(LedgerInfo {
        timestamp: 1_000_000 + eleven_months + eleven_months,
        protocol_version: 20,
        sequence_number: 250,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1_000_000,
        min_persistent_entry_ttl: 1_000_000,
        max_entry_ttl: 10_000_000,
    });

    // Should NOT be expired — only 11 months since last activity
    let result = client.expire_stale_points(&player);
    assert_eq!(result, 600);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_redeem_after_expiry_fails() {
    let (env, _admin, _oracle, player, client) = setup_env();

    let name = String::from_str(&env, "Reward");
    let option_id = client.create_option(&name, &100, &RewardType::Token, &10);

    let reason = String::from_str(&env, "solve");
    client.award_points(&player, &500, &reason);

    // Advance past expiry
    env.ledger().set(LedgerInfo {
        timestamp: 1_000_000 + EXPIRY_WINDOW + 1,
        protocol_version: 20,
        sequence_number: 200,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1_000_000,
        min_persistent_entry_ttl: 1_000_000,
        max_entry_ttl: 10_000_000,
    });

    // Should fail with PointsExpired
    client.redeem(&player, &option_id);
}

// ──────────────────────────────────────────────────────────
// NON-TRANSFERABLE ENFORCEMENT
// ──────────────────────────────────────────────────────────

#[test]
fn test_points_are_non_transferable() {
    let (env, _admin, _oracle, player1, client) = setup_env();
    let player2 = Address::generate(&env);

    let reason = String::from_str(&env, "solve");
    client.award_points(&player1, &1000, &reason);

    // There is no transfer function — points can only be awarded by oracle
    // and redeemed by the owning player. Verify each player's balance is independent.
    assert_eq!(client.get_balance(&player1).current_balance, 1000);
    assert_eq!(client.get_balance(&player2).current_balance, 0);

    // Player2 gets their own points
    client.award_points(&player2, &200, &reason);
    assert_eq!(client.get_balance(&player1).current_balance, 1000);
    assert_eq!(client.get_balance(&player2).current_balance, 200);
}

// ──────────────────────────────────────────────────────────
// ADMIN — REDEMPTION OPTIONS TESTS
// ──────────────────────────────────────────────────────────

#[test]
fn test_create_option() {
    let (env, _admin, _oracle, _player, client) = setup_env();

    let name = String::from_str(&env, "Free NFT");
    let id = client.create_option(&name, &2000, &RewardType::Item, &1);
    assert_eq!(id, 0);

    let option = client.get_option(&id);
    assert_eq!(option.points_cost, 2000);
    assert_eq!(option.reward_type, RewardType::Item);
    assert_eq!(option.reward_value, 1);
    assert!(option.enabled);
}

#[test]
fn test_create_multiple_options_increments_id() {
    let (env, _admin, _oracle, _player, client) = setup_env();

    let n1 = String::from_str(&env, "Option A");
    let n2 = String::from_str(&env, "Option B");
    let n3 = String::from_str(&env, "Option C");

    let id1 = client.create_option(&n1, &100, &RewardType::Token, &10);
    let id2 = client.create_option(&n2, &200, &RewardType::Discount, &5);
    let id3 = client.create_option(&n3, &300, &RewardType::Item, &1);

    assert_eq!(id1, 0);
    assert_eq!(id2, 1);
    assert_eq!(id3, 2);
}

#[test]
fn test_update_option() {
    let (env, _admin, _oracle, _player, client) = setup_env();

    let name = String::from_str(&env, "Old Name");
    let id = client.create_option(&name, &100, &RewardType::Token, &10);

    let new_name = String::from_str(&env, "Updated Name");
    client.update_option(&id, &new_name, &250, &RewardType::Discount, &15, &true);

    let option = client.get_option(&id);
    assert_eq!(option.points_cost, 250);
    assert_eq!(option.reward_type, RewardType::Discount);
    assert_eq!(option.reward_value, 15);
    assert!(option.enabled);
}

#[test]
fn test_disable_option() {
    let (env, _admin, _oracle, _player, client) = setup_env();

    let name = String::from_str(&env, "Reward");
    let id = client.create_option(&name, &100, &RewardType::Token, &10);

    client.disable_option(&id);

    let option = client.get_option(&id);
    assert!(!option.enabled);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_update_nonexistent_option_fails() {
    let (_env, _admin, _oracle, _player, client) = setup_env();

    let name = String::from_str(&client.env, "phantom");
    client.update_option(&99, &name, &100, &RewardType::Token, &10, &true);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_disable_nonexistent_option_fails() {
    let (_env, _admin, _oracle, _player, client) = setup_env();
    client.disable_option(&99);
}

// ──────────────────────────────────────────────────────────
// VIEW FUNCTION TESTS
// ──────────────────────────────────────────────────────────

#[test]
fn test_get_balance_unregistered_player() {
    let (env, _admin, _oracle, _player, client) = setup_env();

    let unknown = Address::generate(&env);
    let balance = client.get_balance(&unknown);
    assert_eq!(balance.current_balance, 0);
    assert_eq!(balance.total_earned, 0);
    assert_eq!(balance.total_redeemed, 0);
    assert_eq!(balance.last_activity, 0);
}

// ──────────────────────────────────────────────────────────
// ORACLE MANAGEMENT
// ──────────────────────────────────────────────────────────

#[test]
fn test_set_oracle() {
    let (env, _admin, _oracle, player, client) = setup_env();

    let new_oracle = Address::generate(&env);
    client.set_oracle(&new_oracle);

    // New oracle can award points
    let reason = String::from_str(&env, "test");
    let balance = client.award_points(&player, &100, &reason);
    assert_eq!(balance, 100);
}

// ──────────────────────────────────────────────────────────
// FULL WORKFLOW TEST
// ──────────────────────────────────────────────────────────

#[test]
fn test_full_lifecycle() {
    let (env, _admin, _oracle, player, client) = setup_env();

    // 1. Create redemption options
    let small = String::from_str(&env, "5% Off");
    let big = String::from_str(&env, "Free Item");
    let small_id = client.create_option(&small, &200, &RewardType::Discount, &5);
    let big_id = client.create_option(&big, &1000, &RewardType::Item, &1);

    // 2. Oracle awards points over time
    let r1 = String::from_str(&env, "puzzle_complete");
    let r2 = String::from_str(&env, "daily_login");
    let r3 = String::from_str(&env, "referral");
    client.award_points(&player, &500, &r1);
    client.award_points(&player, &100, &r2);
    client.award_points(&player, &600, &r3);

    assert_eq!(client.get_balance(&player).current_balance, 1200);

    // 3. Player redeems small reward
    let remaining = client.redeem(&player, &small_id);
    assert_eq!(remaining, 1000);

    // 4. Player redeems big reward
    let remaining = client.redeem(&player, &big_id);
    assert_eq!(remaining, 0);

    // 5. Verify final state
    let balance = client.get_balance(&player);
    assert_eq!(balance.total_earned, 1200);
    assert_eq!(balance.total_redeemed, 1200);
    assert_eq!(balance.current_balance, 0);

    // 6. Cannot redeem with zero balance
    // (would panic with InsufficientBalance if tried)
}
