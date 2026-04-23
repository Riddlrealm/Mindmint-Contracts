#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);

    // Config is private, but we can verify it works by testing oracle authorization
    let player = Address::generate(&env);
    client.create_period(&admin, &1, &String::from_str(&env, "test"));
    client.submit_score(&1, &player, &100); // This would fail if oracle wasn't set correctly
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_double_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);
    client.initialize(&admin, &oracle); // should panic
}

#[test]
fn test_create_period() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);
    client.create_period(&admin, &1, &String::from_str(&env, "season_1_xp"));

    let period = client.get_leaderboard(&1);
    assert_eq!(period.period_id, 1);
    assert_eq!(period.context, String::from_str(&env, "season_1_xp"));
    assert_eq!(period.entries.len(), 0);
    assert!(!period.finalized);
    assert!(period.finalized_at.is_none());

    let periods = client.get_all_periods(&String::from_str(&env, "season_1_xp"));
    assert_eq!(periods.len(), 1);
    assert_eq!(periods.get(0).unwrap(), 1);
}

#[test]
#[should_panic(expected = "Period already exists")]
fn test_duplicate_period() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);
    client.create_period(&admin, &1, &String::from_str(&env, "season_1_xp"));
    client.create_period(&admin, &1, &String::from_str(&env, "season_1_xp")); // should panic
}

#[test]
fn test_submit_score_and_ranking() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let player1 = Address::generate(&env);
    let player2 = Address::generate(&env);
    let player3 = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);
    client.create_period(&admin, &1, &String::from_str(&env, "season_1_xp"));

    // Submit scores in random order
    client.submit_score(&1, &player2, &150);
    client.submit_score(&1, &player1, &200);
    client.submit_score(&1, &player3, &100);

    let period = client.get_leaderboard(&1);
    assert_eq!(period.entries.len(), 3);

    // Check ranking: player1 (200) should be rank 1, player2 (150) rank 2, player3 (100) rank 3
    let entry1 = period.entries.get(0).unwrap();
    assert_eq!(entry1.player, player1);
    assert_eq!(entry1.score, 200);
    assert_eq!(entry1.rank, 1);

    let entry2 = period.entries.get(1).unwrap();
    assert_eq!(entry2.player, player2);
    assert_eq!(entry2.score, 150);
    assert_eq!(entry2.rank, 2);

    let entry3 = period.entries.get(2).unwrap();
    assert_eq!(entry3.player, player3);
    assert_eq!(entry3.score, 100);
    assert_eq!(entry3.rank, 3);
}

#[test]
fn test_player_score_update() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let player1 = Address::generate(&env);
    let player2 = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);
    client.create_period(&admin, &1, &String::from_str(&env, "season_1_xp"));

    // Initial score
    client.submit_score(&1, &player1, &100);
    client.submit_score(&1, &player2, &150);

    let period = client.get_leaderboard(&1);
    assert_eq!(period.entries.len(), 2);
    assert_eq!(period.entries.get(0).unwrap().player, player2); // Higher score first
    assert_eq!(period.entries.get(1).unwrap().player, player1);

    // Update player1 score to be higher
    client.submit_score(&1, &player1, &200);

    let period = client.get_leaderboard(&1);
    assert_eq!(period.entries.len(), 2);
    assert_eq!(period.entries.get(0).unwrap().player, player1); // Now player1 is first
    assert_eq!(period.entries.get(0).unwrap().score, 200);
    assert_eq!(period.entries.get(1).unwrap().player, player2);
    assert_eq!(period.entries.get(1).unwrap().score, 150);
}

#[test]
fn test_top_100_cap() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);
    client.create_period(&admin, &1, &String::from_str(&env, "season_1_xp"));

    // Submit 105 scores
    for i in 0..105 {
        let player = Address::generate(&env);
        client.submit_score(&1, &player, &(105 - i)); // Descending scores
    }

    let period = client.get_leaderboard(&1);
    assert_eq!(period.entries.len(), 100); // Should be capped at 100
}

#[test]
fn test_finalize_period() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let player1 = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);
    client.create_period(&admin, &1, &String::from_str(&env, "season_1_xp"));
    client.submit_score(&1, &player1, &100);

    let period_before = client.get_leaderboard(&1);
    assert!(!period_before.finalized);
    assert!(period_before.finalized_at.is_none());

    client.finalize_period(&admin, &1);

    let period_after = client.get_leaderboard(&1);
    assert!(period_after.finalized);
    assert!(period_after.finalized_at.is_some());
    assert_eq!(period_after.entries.len(), 1); // Entries should be preserved
}

#[test]
#[should_panic(expected = "Already finalized")]
fn test_double_finalize() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);
    client.create_period(&admin, &1, &String::from_str(&env, "season_1_xp"));
    client.finalize_period(&admin, &1);
    client.finalize_period(&admin, &1); // should panic
}

#[test]
#[should_panic(expected = "Period is finalized")]
fn test_submit_after_finalize() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let player1 = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);
    client.create_period(&admin, &1, &String::from_str(&env, "season_1_xp"));
    client.finalize_period(&admin, &1);
    client.submit_score(&1, &player1, &100); // should panic
}

#[test]
fn test_get_player_rank() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let player1 = Address::generate(&env);
    let player2 = Address::generate(&env);
    let player3 = Address::generate(&env);
    let player4 = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);
    client.create_period(&admin, &1, &String::from_str(&env, "season_1_xp"));

    client.submit_score(&1, &player1, &200);
    client.submit_score(&1, &player2, &150);
    client.submit_score(&1, &player3, &100);

    // Test existing players
    let rank1 = client.get_player_rank(&1, &player1).unwrap();
    assert_eq!(rank1, (1, 200));

    let rank2 = client.get_player_rank(&1, &player2).unwrap();
    assert_eq!(rank2, (2, 150));

    let rank3 = client.get_player_rank(&1, &player3).unwrap();
    assert_eq!(rank3, (3, 100));

    // Test non-existing player
    let rank4 = client.get_player_rank(&1, &player4);
    assert!(rank4.is_none());
}

#[test]
fn test_multiple_contexts() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let player1 = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);
    client.create_period(&admin, &1, &String::from_str(&env, "season_1_xp"));
    client.create_period(&admin, &2, &String::from_str(&env, "season_1_xp"));
    client.create_period(&admin, &3, &String::from_str(&env, "tournament_1"));

    let season1_periods = client.get_all_periods(&String::from_str(&env, "season_1_xp"));
    assert_eq!(season1_periods.len(), 2);
    assert!(season1_periods.contains(&1));
    assert!(season1_periods.contains(&2));

    let tournament1_periods = client.get_all_periods(&String::from_str(&env, "tournament_1"));
    assert_eq!(tournament1_periods.len(), 1);
    assert!(tournament1_periods.contains(&3));
}

#[test]
#[should_panic(expected = "Admin only")]
fn test_unauthorized_finalize() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let unauthorized = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);
    client.create_period(&admin, &1, &String::from_str(&env, "season_1_xp"));
    client.finalize_period(&unauthorized, &1); // should panic
}

#[test]
#[should_panic(expected = "Admin only")]
fn test_unauthorized_create_period() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let unauthorized = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);
    client.create_period(&unauthorized, &1, &String::from_str(&env, "season_1_xp")); // should panic
}

#[test]
#[should_panic(expected = "Period not found")]
fn test_get_nonexistent_period() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    let contract_id = env.register_contract(None, ImmutableLeaderboard);
    let client = ImmutableLeaderboardClient::new(&env, &contract_id);

    client.initialize(&admin, &oracle);
    client.get_leaderboard(&1); // should panic
}
