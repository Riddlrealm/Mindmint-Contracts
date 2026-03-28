use soroban_sdk::{Env, Address};
use clawback_reward::{ClawbackRewardContract, RewardStatus};

#[test]
fn test_record_reward() {
    let env = Env::default();
    let recipient = Address::generate(&env);
    let reward_id = ClawbackRewardContract::record_reward(env.clone(), recipient.clone(), 100);
    let record = ClawbackRewardContract::get_reward(env.clone(), reward_id);
    assert_eq!(record.amount, 100);
    assert_eq!(record.status, RewardStatus::Active);
}

#[test]
fn test_flag_and_clawback() {
    let env = Env::default();
    let recipient = Address::generate(&env);
    let reward_id = ClawbackRewardContract::record_reward(env.clone(), recipient.clone(), 200);

    ClawbackRewardContract::flag_reward(env.clone(), reward_id, "fraud".to_string());
    ClawbackRewardContract::execute_clawback(env.clone(), reward_id);

    let record = ClawbackRewardContract::get_reward(env.clone(), reward_id);
    assert_eq!(record.status, RewardStatus::ClawedBack);
}

#[test]
fn test_lock_reward() {
    let env = Env::default();
    let recipient = Address::generate(&env);
    let reward_id = ClawbackRewardContract::record_reward(env.clone(), recipient.clone(), 300);

    ClawbackRewardContract::lock_reward(env.clone(), reward_id);
    let record = ClawbackRewardContract::get_reward(env.clone(), reward_id);
    assert_eq!(record.status, RewardStatus::Locked);
}
