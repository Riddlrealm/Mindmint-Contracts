#![no_std]

mod types;

use soroban_sdk::{contract, contractimpl, Address, Env};
use types::{Config, ContractError, DataKey, Feedback, Milestone, ReputationScore};

#[contract]
pub struct ReputationContract;

#[contractimpl]
impl ReputationContract {
    /// Initialize the reputation contract with configuration
    pub fn initialize(
        env: Env,
        admin: Address,
        decay_rate: u32,
        decay_period: u64,
        min_feedback_gap: u64,
        recovery_cap: u32,
    ) -> Result<(), ContractError> {
        // Check if already initialized
        if env.storage().instance().has(&DataKey::Config) {
            return Err(ContractError::AlreadyInitialized);
        }

        // Create configuration
        let config = Config {
            admin: admin.clone(),
            decay_rate,
            decay_period,
            min_feedback_gap,
            recovery_cap,
        };

        // Save configuration to persistent storage
        env.storage().instance().set(&DataKey::Config, &config);

        // Set default milestones
        Self::set_default_milestones(&env);

        Ok(())
    }

    /// Record feedback from one player to another
    pub fn record_feedback(
        env: Env,
        from: Address,
        to: Address,
        is_positive: bool,
        weight: u32,
        reason: u32,
    ) -> Result<(), ContractError> {
        // Require authentication
        from.require_auth();

        // Validate that sender is not giving feedback to themselves
        if from == to {
            return Err(ContractError::SelfFeedback);
        }

        // Check rate limit
        Self::check_feedback_rate_limit(&env, &from, &to)?;

        // Get current feedback count
        let feedback_count = Self::get_feedback_count(&env, &to);

        // Create feedback record
        let feedback = Feedback {
            from: from.clone(),
            to: to.clone(),
            is_positive,
            weight,
            timestamp: env.ledger().timestamp(),
            reason,
        };

        // Save feedback to persistent storage
        env.storage()
            .persistent()
            .set(&DataKey::Feedback(to.clone(), feedback_count), &feedback);

        // Increment feedback count
        env.storage()
            .persistent()
            .set(&DataKey::FeedbackCount(to.clone()), &(feedback_count + 1));

        // Update reputation score
        Self::update_reputation(&env, &to, is_positive, weight)?;

        Ok(())
    }
}

// Helper functions
impl ReputationContract {
    /// Set default milestone levels
    fn set_default_milestones(env: &Env) {
        let milestones = vec![
            env,
            Milestone {
                level: 1,
                score_required: 100,
                badge_id: 1,
                features_unlocked: 1,
            },
            Milestone {
                level: 2,
                score_required: 300,
                badge_id: 2,
                features_unlocked: 3,
            },
            Milestone {
                level: 3,
                score_required: 600,
                badge_id: 3,
                features_unlocked: 7,
            },
            Milestone {
                level: 4,
                score_required: 850,
                badge_id: 4,
                features_unlocked: 15,
            },
        ];

        for milestone in milestones.iter() {
            env.storage()
                .persistent()
                .set(&DataKey::Milestone(milestone.level), &milestone);
        }
    }

    /// Check feedback rate limit to prevent spam
    fn check_feedback_rate_limit(
        env: &Env,
        from: &Address,
        to: &Address,
    ) -> Result<(), ContractError> {
        let config = Self::get_config(env)?;
        let feedback_count = Self::get_feedback_count(env, to);
        
        // Check recent feedbacks from same sender
        for i in 0..feedback_count {
            if let Some(feedback) = env
                .storage()
                .persistent()
                .get::<DataKey, Feedback>(&DataKey::Feedback(to.clone(), i))
            {
                if feedback.from == *from {
                    let time_since_last = env.ledger().timestamp() - feedback.timestamp;
                    if time_since_last < config.min_feedback_gap {
                        return Err(ContractError::RateLimitExceeded);
                    }
                }
            }
        }

        Ok(())
    }

    /// Update player's reputation score
    fn update_reputation(
        env: &Env,
        player: &Address,
        is_positive: bool,
        weight: u32,
    ) -> Result<(), ContractError> {
        let mut reputation = Self::get_or_create_reputation(env, player);

        // Update feedback counters
        if is_positive {
            reputation.positive_feedback += 1;
            reputation.total_score += weight;
        } else {
            reputation.negative_feedback += 1;
            // Subtract weight but don't go below zero
            reputation.total_score = reputation.total_score.saturating_sub(weight);
        }

        // Update last activity timestamp
        reputation.last_activity = env.ledger().timestamp();

        // Save updated reputation
        env.storage()
            .persistent()
            .set(&DataKey::Reputation(player.clone()), &reputation);

        Ok(())
    }

    /// Get configuration from storage
    fn get_config(env: &Env) -> Result<Config, ContractError> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(ContractError::NotInitialized)
    }

    /// Get feedback count for a player
    fn get_feedback_count(env: &Env, player: &Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::FeedbackCount(player.clone()))
            .unwrap_or(0)
    }

    /// Get existing reputation or create new one
    fn get_or_create_reputation(env: &Env, player: &Address) -> ReputationScore {
        env.storage()
            .persistent()
            .get(&DataKey::Reputation(player.clone()))
            .unwrap_or(ReputationScore {
                total_score: 0,
                positive_feedback: 0,
                negative_feedback: 0,
                quests_completed: 0,
                contributions: 0,
                last_activity: env.ledger().timestamp(),
                created_at: env.ledger().timestamp(),
            })
    }
}
