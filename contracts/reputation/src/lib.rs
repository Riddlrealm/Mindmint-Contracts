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

    /// Get player's reputation score with decay applied
    pub fn get_reputation(env: Env, player: Address) -> ReputationScore {
        let mut reputation = Self::get_or_create_reputation(&env, &player);
        
        // Apply decay before returning
        Self::apply_decay(&env, &mut reputation);
        
        // Save updated reputation after decay
        env.storage()
            .persistent()
            .set(&DataKey::Reputation(player), &reputation);
        
        reputation
    }

    /// Calculate weighted reputation score
    pub fn calculate_score(env: Env, player: Address) -> u32 {
        let reputation = Self::get_reputation(env.clone(), player);
        
        // Calculate activity score
        let activity_score = Self::calculate_activity_score(&env, &reputation);
        
        // Weighted score calculation:
        // - Positive feedback: 40%
        // - Quests completed: 30%
        // - Contributions: 20%
        // - Activity: 10%
        let score = (reputation.positive_feedback * 40 / 100)
            + (reputation.quests_completed * 30 / 100)
            + (reputation.contributions * 20 / 100)
            + (activity_score * 10 / 100);
        
        score
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

    /// Apply time-based decay to reputation score
    fn apply_decay(env: &Env, reputation: &mut ReputationScore) {
        let config = match Self::get_config(env) {
            Ok(c) => c,
            Err(_) => return, // Skip decay if not initialized
        };

        let current_time = env.ledger().timestamp();
        let time_elapsed = current_time.saturating_sub(reputation.last_activity);
        
        // Calculate number of decay periods that have passed
        if config.decay_period > 0 && time_elapsed >= config.decay_period {
            let periods_elapsed = time_elapsed / config.decay_period;
            
            // Apply decay: reduce score by decay_rate (in basis points) per period
            // decay_rate is in basis points (e.g., 200 = 2%)
            for _ in 0..periods_elapsed {
                let decay_amount = (reputation.total_score * config.decay_rate) / 10000;
                reputation.total_score = reputation.total_score.saturating_sub(decay_amount);
            }
            
            // Update last activity to current time
            reputation.last_activity = current_time;
        }
    }

    /// Calculate activity score based on last activity
    fn calculate_activity_score(env: &Env, reputation: &ReputationScore) -> u32 {
        let current_time = env.ledger().timestamp();
        let time_since_activity = current_time.saturating_sub(reputation.last_activity);
        
        // Simple activity scoring:
        // - Active (within 7 days): 100 points
        // - Inactive: 50 points
        const SEVEN_DAYS: u64 = 7 * 24 * 60 * 60;
        
        if time_since_activity < SEVEN_DAYS {
            100
        } else {
            50
        }
    }
}
