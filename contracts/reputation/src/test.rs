#![cfg(test)]

use crate::{ReputationContract, ReputationContractClient};
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env};

fn create_test_env<'a>() -> (Env, ReputationContractClient<'a>, Address, Address, Address) {
    let env = Env::default();
    let contract_id = env.register_contract(None, ReputationContract);
    let client = ReputationContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let player1 = Address::generate(&env);
    let player2 = Address::generate(&env);
    
    (env, client, admin, player1, player2)
}

#[test]
fn test_initialization() {
    let (env, client, admin, _, _) = create_test_env();
    env.mock_all_auths();
    
    client.initialize(&admin, &200, &86400, &3600, &50);
    
    let reputation = client.get_reputation(&admin);
    assert_eq!(reputation.total_score, 0);
}

#[test]
#[should_panic]
fn test_double_initialization() {
    let (env, client, admin, _, _) = create_test_env();
    env.mock_all_auths();
    
    client.initialize(&admin, &200, &86400, &3600, &50);
    client.initialize(&admin, &200, &86400, &3600, &50);
}

#[test]
fn test_feedback_recording() {
    let (env, client, admin, player1, player2) = create_test_env();
    env.mock_all_auths();
    
    client.initialize(&admin, &200, &86400, &3600, &50);
    client.record_feedback(&player1, &player2, &true, &10, &1);
    
    let reputation = client.get_reputation(&player2);
    assert_eq!(reputation.positive_feedback, 1);
    assert_eq!(reputation.total_score, 10);
}

#[test]
#[should_panic]
fn test_self_feedback_prevention() {
    let (env, client, admin, player1, _) = create_test_env();
    env.mock_all_auths();
    
    client.initialize(&admin, &200, &86400, &3600, &50);
    client.record_feedback(&player1, &player1, &true, &10, &1);
}

#[test]
#[should_panic]
fn test_rate_limiting() {
    let (env, client, admin, player1, player2) = create_test_env();
    env.mock_all_auths();
    
    client.initialize(&admin, &200, &86400, &3600, &50);
    
    client.record_feedback(&player1, &player2, &true, &10, &1);
    client.record_feedback(&player1, &player2, &true, &10, &1);
}

#[test]
fn test_reputation_calculation() {
    let (env, client, admin, player1, _player2) = create_test_env();
    env.mock_all_auths();
    
    client.initialize(&admin, &200, &86400, &3600, &50);
    
    client.record_quest_completion(&player1, &100);
    client.record_contribution(&player1, &50);
    
    let score = client.calculate_score(&player1);
    assert!(score > 0);
    
    let reputation = client.get_reputation(&player1);
    assert_eq!(reputation.quests_completed, 100);
    assert_eq!(reputation.contributions, 50);
}

#[test]
fn test_milestone_achievement() {
    let (env, client, admin, player1, player2) = create_test_env();
    env.mock_all_auths();
    
    client.initialize(&admin, &200, &86400, &3600, &50);
    
    for _ in 0..10 {
        client.record_quest_completion(&player1, &15);
    }
    
    assert!(client.has_milestone(&player1, &1));
    
    client.record_quest_completion(&player1, &200);
    assert!(client.has_milestone(&player1, &2));
}

#[test]
fn test_reputation_decay() {
    let (env, client, admin, player1, _) = create_test_env();
    env.mock_all_auths();
    
    client.initialize(&admin, &200, &86400, &3600, &50);
    client.record_quest_completion(&player1, &100);
    
    let initial_reputation = client.get_reputation(&player1);
    let initial_score = initial_reputation.total_score;
    
    env.ledger().with_mut(|li| {
        li.timestamp = li.timestamp + 86400 * 2;
    });
    
    let decayed_reputation = client.get_reputation(&player1);
    assert!(decayed_reputation.total_score < initial_score);
}

#[test]
fn test_reputation_recovery() {
    let (env, client, admin, player1, player2) = create_test_env();
    env.mock_all_auths();
    
    client.initialize(&admin, &200, &86400, &3600, &50);
    
    client.record_feedback(&player2, &player1, &false, &20, &1);
    
    let reputation_before = client.get_reputation(&player1);
    let negative_before = reputation_before.negative_feedback;
    
    client.request_recovery(&player1, &30);
    
    let reputation_after = client.get_reputation(&player1);
    assert!(reputation_after.total_score > reputation_before.total_score);
    assert!(reputation_after.negative_feedback < negative_before);
}

#[test]
fn test_reputation_recovery_cap() {
    let (env, client, admin, player1, _) = create_test_env();
    env.mock_all_auths();
    
    client.initialize(&admin, &200, &86400, &3600, &50);
    
    let initial_reputation = client.get_reputation(&player1);
    let initial_score = initial_reputation.total_score;
    
    client.request_recovery(&player1, &100);
    
    let final_reputation = client.get_reputation(&player1);
    assert_eq!(final_reputation.total_score, initial_score + 50);
}

#[test]
fn test_negative_feedback_impact() {
    let (env, client, admin, player1, player2) = create_test_env();
    env.mock_all_auths();
    
    client.initialize(&admin, &200, &86400, &3600, &50);
    
    client.record_quest_completion(&player1, &100);
    let reputation_before = client.get_reputation(&player1);
    
    client.record_feedback(&player2, &player1, &false, &30, &2);
    
    let reputation_after = client.get_reputation(&player1);
    assert_eq!(reputation_after.negative_feedback, 1);
    assert!(reputation_after.total_score < reputation_before.total_score);
}
