use soroban_sdk::{contractimpl, Address, Env, Symbol};

#[derive(Clone)]
pub struct RewardRecord {
    pub id: u64,
    pub recipient: Address,
    pub amount: i128,
    pub issued_at: u64,
    pub status: RewardStatus,
    pub flag_reason: Option<String>,
}

#[derive(Clone)]
pub enum RewardStatus {
    Active,
    ClawedBack,
    Locked,
}

pub struct ClawbackRewardContract;

#[contractimpl]
impl ClawbackRewardContract {
    pub fn record_reward(env: Env, recipient: Address, amount: i128) -> u64 {
        // generate reward_id, store record
        // set status = Active
        // set issued_at = env.ledger().timestamp()
        // return reward_id
        0
    }

    pub fn flag_reward(env: Env, reward_id: u64, reason: String) {
        // only admin can call
        // check dispute window
        // update record flag_reason
        // emit RewardFlagged event
    }

    pub fn execute_clawback(env: Env, reward_id: u64) {
        // only admin can call
        // check flagged + within window
        // transfer tokens back to pool
        // update status = ClawedBack
        // emit RewardClawedBack event
    }

    pub fn lock_reward(env: Env, reward_id: u64) {
        // check dispute window passed
        // update status = Locked
    }

    pub fn get_reward(env: Env, reward_id: u64) -> RewardRecord {
        // return full record
        RewardRecord {
            id: reward_id,
            recipient: Address::generate(&env),
            amount: 0,
            issued_at: env.ledger().timestamp(),
            status: RewardStatus::Active,
            flag_reason: None,
        }
    }

    pub fn get_clawback_history(env: Env, recipient: Address) -> Vec<RewardRecord> {
        // query all records for recipient with status = ClawedBack
        Vec::new()
    }
}

// Events
pub fn emit_reward_flagged(env: &Env, reward_id: u64, reason: &str) {
    env.events().publish(
        (Symbol::short("RewardFlagged"), reward_id),
        reason,
    );
}

pub fn emit_reward_clawed_back(env: &Env, reward_id: u64, amount: i128) {
    env.events().publish(
        (Symbol::short("RewardClawedBack"), reward_id),
        amount,
    );
}
