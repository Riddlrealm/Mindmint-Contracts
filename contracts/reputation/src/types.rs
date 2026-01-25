use soroban_sdk::{contracttype, Address};

/// Represents a user's reputation score and related metrics
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReputationScore {
    pub total_score: u32,
    pub positive_feedback: u32,
    pub negative_feedback: u32,
    pub quests_completed: u32,
    pub contributions: u32,
    pub last_activity: u64,
    pub created_at: u64,
}

/// Represents feedback given from one user to another
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feedback {
    pub from: Address,
    pub to: Address,
    pub is_positive: bool,
    pub weight: u32,
    pub timestamp: u64,
    pub reason: u32,
}

/// Represents a reputation milestone that unlocks features
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Milestone {
    pub level: u32,
    pub score_required: u32,
    pub badge_id: u32,
    pub features_unlocked: u32,
}

/// Contract configuration parameters
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub admin: Address,
    pub decay_rate: u32,
    pub decay_period: u64,
    pub min_feedback_gap: u64,
    pub recovery_cap: u32,
}

/// Storage keys for the contract data
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataKey {
    Config,
    Reputation(Address),
    Feedback(Address, u32),
    FeedbackCount(Address),
    Milestone(u32),
    PlayerMilestones(Address),
}
