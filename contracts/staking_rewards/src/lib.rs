#![no_std]

mod storage;
mod events;
#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, Address, Env, panic_with_error,
    token::Client as TokenClient,
};
use crate::storage::*;
use crate::events::*;

#[contract]
pub struct StakingRewardsContract;

#[contractimpl]
impl StakingRewardsContract {
    /// Initialize the staking pool with configuration
    pub fn initialize(
        env: Env,
        admin: Address,
        staking_token: Address,
        reward_token: Address,
        apy_bps: u32, // APY in basis points (100 = 1%)
        lockup_period: u64, // Lockup period in seconds
        early_unstake_penalty_bps: u32, // Penalty for early unstake (bps)
        auto_compound_default: bool,
    ) {
        if env.storage().instance().has(&DataKey::Initialized) {
            panic!("Staking pool already initialized");
        }

        if apy_bps > 10000 {
            panic_with_error!(&env, 1); // Max 100% APY
        }
        if early_unstake_penalty_bps > 10000 {
            panic_with_error!(&env, 2); // Max 100% penalty
        }

        set_admin(&env, &admin);
        set_staking_token(&env, &staking_token);
        set_reward_token(&env, &reward_token);
        set_apy_bps(&env, &apy_bps);
        set_lockup_period(&env, &lockup_period);
        set_early_unstake_penalty_bps(&env, &early_unstake_penalty_bps);
        set_auto_compound_default(&env, &auto_compound_default);
        set_total_staked(&env, &0);
        env.storage().instance().set(&DataKey::Initialized, &true);

        emit_pool_initialized(&env, staking_token, reward_token, apy_bps, lockup_period);
    }

    /// Stake tokens into the pool
    pub fn stake(env: Env, user: Address, amount: i128, auto_compound: Option<bool>) {
        user.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, 3); // Invalid amount
        }

        // Calculate and claim any pending rewards before adding new stake
        let mut position = get_position(&env, &user);
        if position.staked_amount > 0 {
            let pending = calculate_rewards(&env, &user);
            if pending > 0 {
                position.pending_rewards += pending;
            }
        }

        // Transfer staking tokens from user
        let staking_token = get_staking_token(&env);
        let token_client = TokenClient::new(&env, &staking_token);
        token_client.transfer(&user, &env.current_contract_address(), &amount);

        // Update position
        let current_time = env.ledger().timestamp();
        position.staked_amount += amount;
        position.staked_at = current_time; // Reset stake time for new deposit
        position.last_update_time = current_time;
        position.auto_compound = auto_compound.unwrap_or(get_auto_compound_default(&env));
        
        // Add to staking history
        add_staking_history(&env, &user, amount, current_time, StakingAction::Stake);

        // Update global state
        let total_staked = get_total_staked(&env);
        set_total_staked(&env, &(total_staked + amount));

        set_position(&env, &user, &position);

        emit_staked(&env, user, amount, current_time);
    }

    /// Unstake tokens from the pool
    pub fn unstake(env: Env, user: Address, amount: i128) -> i128 {
        user.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, 4); // Invalid amount
        }

        let mut position = get_position(&env, &user);
        if position.staked_amount < amount {
            panic_with_error!(&env, 5); // Insufficient staked balance
        }

        // Calculate pending rewards before unstaking
        let pending = calculate_rewards(&env, &user);
        position.pending_rewards += pending;
        position.last_update_time = env.ledger().timestamp();

        // Check lockup period and apply penalty if needed
        let lockup_period = get_lockup_period(&env);
        let current_time = env.ledger().timestamp();
        let unstaked_amount = if current_time < position.staked_at + lockup_period {
            // Apply early unstake penalty
            let penalty_bps = get_early_unstake_penalty_bps(&env);
            let penalty = amount * penalty_bps as i128 / 10000;
            let actual_amount = amount - penalty;
            
            // Penalty stays in the contract (goes to reward pool)
            add_staking_history(&env, &user, penalty, current_time, StakingAction::Penalty);
            
            actual_amount
        } else {
            amount
        };

        // Update position
        position.staked_amount -= amount;

        // Update global state
        let total_staked = get_total_staked(&env);
        set_total_staked(&env, &(total_staked - amount));

        // Transfer tokens back to user
        let staking_token = get_staking_token(&env);
        let token_client = TokenClient::new(&env, &staking_token);
        token_client.transfer(&env.current_contract_address(), &user, &unstaked_amount);

        // Record unstake in history
        add_staking_history(&env, &user, unstaked_amount, current_time, StakingAction::Unstake);

        set_position(&env, &user, &position);

        emit_unstaked(&env, user, unstaked_amount, current_time);

        unstaked_amount
    }

    /// Claim accumulated rewards
    pub fn claim_rewards(env: Env, user: Address) -> i128 {
        user.require_auth();

        let mut position = get_position(&env, &user);
        let current_time = env.ledger().timestamp();

        // Calculate all pending rewards
        let pending = calculate_rewards(&env, &user);
        let total_claimable = position.pending_rewards + pending;

        if total_claimable <= 0 {
            panic_with_error!(&env, 6); // No rewards to claim
        }

        // Handle auto-compounding if enabled
        if position.auto_compound {
            // Convert rewards to staking tokens (if they're the same) or restake
            let staking_token = get_staking_token(&env);
            let reward_token = get_reward_token(&env);
            
            if staking_token == reward_token {
                // Auto-compound by adding rewards to staked amount
                position.staked_amount += total_claimable;
                let total_staked = get_total_staked(&env);
                set_total_staked(&env, &(total_staked + total_claimable));
                add_staking_history(&env, &user, total_claimable, current_time, StakingAction::Compound);
            }
        } else {
            // Transfer rewards to user
            let reward_token = get_reward_token(&env);
            let token_client = TokenClient::new(&env, &reward_token);
            token_client.transfer(&env.current_contract_address(), &user, &total_claimable);
        }

        // Reset pending rewards and update timestamp
        position.pending_rewards = 0;
        position.last_update_time = current_time;
        position.total_claimed += total_claimable;

        add_staking_history(&env, &user, total_claimable, current_time, StakingAction::Claim);
        set_position(&env, &user, &position);

        emit_rewards_claimed(&env, user, total_claimable, current_time, position.auto_compound);

        total_claimable
    }

    /// Update APY (only admin)
    pub fn update_apy(env: Env, new_apy_bps: u32) {
        let admin = get_admin(&env);
        admin.require_auth();

        if new_apy_bps > 10000 {
            panic_with_error!(&env, 7); // Max 100% APY
        }

        // Distribute all pending rewards before changing APY
        snap_all_rewards(&env);
        
        set_apy_bps(&env, &new_apy_bps);
        emit_apy_updated(&env, new_apy_bps);
    }

    /// Fund the reward pool (only admin)
    pub fn fund_reward_pool(env: Env, amount: i128) {
        let admin = get_admin(&env);
        admin.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, 8); // Invalid amount
        }

        let reward_token = get_reward_token(&env);
        let token_client = TokenClient::new(&env, &reward_token);
        token_client.transfer(&admin, &env.current_contract_address(), &amount);

        emit_pool_funded(&env, amount);
    }

    /// Toggle auto-compound for a user
    pub fn toggle_auto_compound(env: Env, user: Address, enabled: bool) {
        user.require_auth();

        // Claim pending rewards before changing setting
        let mut position = get_position(&env, &user);
        let pending = calculate_rewards(&env, &user);
        position.pending_rewards += pending;
        position.last_update_time = env.ledger().timestamp();
        position.auto_compound = enabled;

        set_position(&env, &user, &position);
        emit_auto_compound_toggled(&env, user, enabled);
    }

    // View functions
    pub fn get_staked_amount(env: Env, user: Address) -> i128 {
        let position = get_position(&env, &user);
        position.staked_amount
    }

    pub fn get_pending_rewards(env: Env, user: Address) -> i128 {
        let position = get_position(&env, &user);
        let pending = calculate_rewards(&env, &user);
        position.pending_rewards + pending
    }

    pub fn get_total_staked(env: Env) -> i128 {
        get_total_staked(&env)
    }

    pub fn get_position(env: Env, user: Address) -> StakingPosition {
        get_position(&env, &user)
    }

    pub fn get_staking_history(env: Env, user: Address) -> Vec<StakingHistoryEntry> {
        get_staking_history(&env, &user)
    }

    pub fn get_config(env: Env) -> StakingConfig {
        StakingConfig {
            admin: get_admin(&env),
            staking_token: get_staking_token(&env),
            reward_token: get_reward_token(&env),
            apy_bps: get_apy_bps(&env),
            lockup_period: get_lockup_period(&env),
            early_unstake_penalty_bps: get_early_unstake_penalty_bps(&env),
            auto_compound_default: get_auto_compound_default(&env),
        }
    }
}

#[contracttype]
pub struct StakingConfig {
    pub admin: Address,
    pub staking_token: Address,
    pub reward_token: Address,
    pub apy_bps: u32,
    pub lockup_period: u64,
    pub early_unstake_penalty_bps: u32,
    pub auto_compound_default: bool,
}