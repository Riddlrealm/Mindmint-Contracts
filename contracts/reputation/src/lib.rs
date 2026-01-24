#![no_std]

mod types;

use soroban_sdk::{contract, contractimpl, Address, Env};
use types::{Config, ContractError, DataKey, Milestone};

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
}
