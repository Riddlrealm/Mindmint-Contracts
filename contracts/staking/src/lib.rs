#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Vec};

//
// ──────────────────────────────────────────────────────────
// STAKING TIERS
// ──────────────────────────────────────────────────────────
//

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StakingTier {
    None = 0,
    Bronze = 1,
    Silver = 2,
    Gold = 3,
}

//
// ──────────────────────────────────────────────────────────
// DATA KEYS
// ──────────────────────────────────────────────────────────
//

#[contracttype]
pub enum DataKey {
    Config,                    // StakingConfig
    StakerInfo(Address),       // StakerInfo
    StakersList,               // Vec<Address>
    TotalStaked,               // i128
    RewardPool,                // i128
}

//
// ──────────────────────────────────────────────────────────
// STRUCTS
// ──────────────────────────────────────────────────────────
//

#[contracttype]
#[derive(Clone, Debug)]
pub struct StakingConfig {
    pub admin: Address,
    pub staking_token: Address,
    pub reward_token: Address,
    pub base_apy: u32,              // Base APY in basis points (100 = 1%)
    pub bronze_bonus: u32,          // Additional APY for Bronze tier
    pub silver_bonus: u32,          // Additional APY for Silver tier
    pub gold_bonus: u32,            // Additional APY for Gold tier
    pub bronze_threshold: i128,     // Min stake for Bronze
    pub silver_threshold: i128,     // Min stake for Silver
    pub gold_threshold: i128,       // Min stake for Gold
    pub min_lock_period: u64,       // Minimum lock period in seconds
    pub early_unstake_penalty: u32, // Penalty in basis points (1000 = 10%)
    pub emergency_penalty: u32,     // Emergency withdrawal penalty
    pub paused: bool,               // Contract pause state
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct StakerInfo {
    pub staked_amount: i128,
    pub stake_timestamp: u64,
    pub last_reward_claim: u64,
    pub accumulated_rewards: i128,
    pub tier: StakingTier,
}

//
// ──────────────────────────────────────────────────────────
// CONSTANTS
// ──────────────────────────────────────────────────────────
//

const SECONDS_PER_YEAR: u64 = 31_536_000;
const BASIS_POINTS: u64 = 10_000;

//
// ──────────────────────────────────────────────────────────
// CONTRACT
// ──────────────────────────────────────────────────────────
//

#[contract]
pub struct StakingContract;

#[contractimpl]
impl StakingContract {
    // ───────────── INITIALIZATION ─────────────

    /// Initialize the staking contract with configuration
    ///
    /// # Arguments
    /// * `admin` - Contract administrator
    /// * `staking_token` - Token address that users will stake
    /// * `reward_token` - Token address for reward distribution
    /// * `base_apy` - Base annual percentage yield in basis points (500 = 5%)
    /// * `min_lock_period` - Minimum staking period in seconds
    pub fn initialize(
        env: Env,
        admin: Address,
        staking_token: Address,
        reward_token: Address,
        base_apy: u32,
        min_lock_period: u64,
    ) {
        admin.require_auth();

        if env.storage().persistent().has(&DataKey::Config) {
            panic!("Already initialized");
        }

        let config = StakingConfig {
            admin,
            staking_token,
            reward_token,
            base_apy,
            bronze_bonus: 100,       // +1% APY for Bronze
            silver_bonus: 250,       // +2.5% APY for Silver
            gold_bonus: 500,         // +5% APY for Gold
            bronze_threshold: 1_000_000_000,     // 1,000 tokens (assuming 6 decimals)
            silver_threshold: 10_000_000_000,    // 10,000 tokens
            gold_threshold: 100_000_000_000,     // 100,000 tokens
            min_lock_period,
            early_unstake_penalty: 1_000,  // 10% penalty
            emergency_penalty: 2_000,      // 20% penalty
            paused: false,
        };

        env.storage().persistent().set(&DataKey::Config, &config);
        env.storage().persistent().set(&DataKey::TotalStaked, &0i128);
        env.storage().persistent().set(&DataKey::RewardPool, &0i128);
    }

    // ───────────── ADMIN FUNCTIONS ─────────────

    /// Update APY configuration (admin only)
    pub fn update_apy_config(
        env: Env,
        admin: Address,
        base_apy: u32,
        bronze_bonus: u32,
        silver_bonus: u32,
        gold_bonus: u32,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut config: StakingConfig = env.storage().persistent().get(&DataKey::Config).unwrap();

        config.base_apy = base_apy;
        config.bronze_bonus = bronze_bonus;
        config.silver_bonus = silver_bonus;
        config.gold_bonus = gold_bonus;

        env.storage().persistent().set(&DataKey::Config, &config);
    }

    /// Update tier thresholds (admin only)
    pub fn update_tier_thresholds(
        env: Env,
        admin: Address,
        bronze_threshold: i128,
        silver_threshold: i128,
        gold_threshold: i128,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut config: StakingConfig = env.storage().persistent().get(&DataKey::Config).unwrap();

        config.bronze_threshold = bronze_threshold;
        config.silver_threshold = silver_threshold;
        config.gold_threshold = gold_threshold;

        env.storage().persistent().set(&DataKey::Config, &config);
    }

    /// Update staking parameters (admin only)
    pub fn update_staking_params(
        env: Env,
        admin: Address,
        min_lock_period: u64,
        early_unstake_penalty: u32,
        emergency_penalty: u32,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut config: StakingConfig = env.storage().persistent().get(&DataKey::Config).unwrap();

        config.min_lock_period = min_lock_period;
        config.early_unstake_penalty = early_unstake_penalty;
        config.emergency_penalty = emergency_penalty;

        env.storage().persistent().set(&DataKey::Config, &config);
    }

    /// Pause/unpause the contract (admin only)
    pub fn set_paused(env: Env, admin: Address, paused: bool) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut config: StakingConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        config.paused = paused;
        env.storage().persistent().set(&DataKey::Config, &config);
    }

    /// Add rewards to the reward pool (admin only)
    pub fn add_rewards(env: Env, admin: Address, amount: i128) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let config: StakingConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let reward_client = token::Client::new(&env, &config.reward_token);

        reward_client.transfer(&admin, &env.current_contract_address(), &amount);

        let current_pool: i128 = env.storage().persistent().get(&DataKey::RewardPool).unwrap_or(0);
        env.storage().persistent().set(&DataKey::RewardPool, &(current_pool + amount));
    }

    // ───────────── STAKING FUNCTIONS ─────────────

    /// Stake tokens
    pub fn stake(env: Env, staker: Address, amount: i128) {
        staker.require_auth();
        Self::assert_not_paused(&env);

        if amount <= 0 {
            panic!("Amount must be positive");
        }

        let config: StakingConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let staking_client = token::Client::new(&env, &config.staking_token);

        // Transfer tokens from staker to contract
        staking_client.transfer(&staker, &env.current_contract_address(), &amount);

        // Get or create staker info
        let mut staker_info = Self::get_staker_info(env.clone(), staker.clone())
            .unwrap_or(StakerInfo {
                staked_amount: 0,
                stake_timestamp: env.ledger().timestamp(),
                last_reward_claim: env.ledger().timestamp(),
                accumulated_rewards: 0,
                tier: StakingTier::None,
            });

        // If existing stake, claim pending rewards first
        if staker_info.staked_amount > 0 {
            let pending = Self::calculate_pending_rewards(&env, &staker_info, &config);
            staker_info.accumulated_rewards += pending;
        }

        // Update staker info
        staker_info.staked_amount += amount;
        staker_info.stake_timestamp = env.ledger().timestamp();
        staker_info.last_reward_claim = env.ledger().timestamp();
        staker_info.tier = Self::calculate_tier(staker_info.staked_amount, &config);

        env.storage().persistent().set(&DataKey::StakerInfo(staker.clone()), &staker_info);

        // Update stakers list
        Self::add_to_stakers_list(&env, staker);

        // Update total staked
        let total_staked: i128 = env.storage().persistent().get(&DataKey::TotalStaked).unwrap_or(0);
        env.storage().persistent().set(&DataKey::TotalStaked, &(total_staked + amount));
    }

    /// Unstake tokens (with early unstake penalty if applicable)
    pub fn unstake(env: Env, staker: Address, amount: i128) {
        staker.require_auth();
        Self::assert_not_paused(&env);

        if amount <= 0 {
            panic!("Amount must be positive");
        }

        let config: StakingConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let mut staker_info: StakerInfo = env.storage().persistent()
            .get(&DataKey::StakerInfo(staker.clone()))
            .expect("Not staked");

        if staker_info.staked_amount < amount {
            panic!("Insufficient staked balance");
        }

        // Calculate pending rewards before unstaking
        let pending = Self::calculate_pending_rewards(&env, &staker_info, &config);
        staker_info.accumulated_rewards += pending;

        // Check if early unstake (before lock period ends)
        let time_staked = env.ledger().timestamp() - staker_info.stake_timestamp;
        let mut penalty_amount: i128 = 0;

        if time_staked < config.min_lock_period {
            penalty_amount = (amount * config.early_unstake_penalty as i128) / BASIS_POINTS as i128;
        }

        let amount_to_return = amount - penalty_amount;

        // Update staker info
        staker_info.staked_amount -= amount;
        staker_info.last_reward_claim = env.ledger().timestamp();
        staker_info.tier = Self::calculate_tier(staker_info.staked_amount, &config);

        env.storage().persistent().set(&DataKey::StakerInfo(staker.clone()), &staker_info);

        // Transfer tokens back to staker
        let staking_client = token::Client::new(&env, &config.staking_token);
        staking_client.transfer(&env.current_contract_address(), &staker, &amount_to_return);

        // Update total staked
        let total_staked: i128 = env.storage().persistent().get(&DataKey::TotalStaked).unwrap_or(0);
        env.storage().persistent().set(&DataKey::TotalStaked, &(total_staked - amount));

        // Remove from stakers list if fully unstaked
        if staker_info.staked_amount == 0 {
            Self::remove_from_stakers_list(&env, staker);
        }
    }

    /// Claim accumulated rewards
    pub fn claim_rewards(env: Env, staker: Address) -> i128 {
        staker.require_auth();
        Self::assert_not_paused(&env);

        let config: StakingConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let mut staker_info: StakerInfo = env.storage().persistent()
            .get(&DataKey::StakerInfo(staker.clone()))
            .expect("Not staked");

        // Calculate total rewards
        let pending = Self::calculate_pending_rewards(&env, &staker_info, &config);
        let total_rewards = staker_info.accumulated_rewards + pending;

        if total_rewards <= 0 {
            panic!("No rewards to claim");
        }

        // Check reward pool has enough
        let reward_pool: i128 = env.storage().persistent().get(&DataKey::RewardPool).unwrap_or(0);
        if reward_pool < total_rewards {
            panic!("Insufficient reward pool");
        }

        // Update staker info
        staker_info.accumulated_rewards = 0;
        staker_info.last_reward_claim = env.ledger().timestamp();
        env.storage().persistent().set(&DataKey::StakerInfo(staker.clone()), &staker_info);

        // Update reward pool
        env.storage().persistent().set(&DataKey::RewardPool, &(reward_pool - total_rewards));

        // Transfer rewards
        let reward_client = token::Client::new(&env, &config.reward_token);
        reward_client.transfer(&env.current_contract_address(), &staker, &total_rewards);

        total_rewards
    }

    /// Emergency withdrawal - withdraw all staked tokens with higher penalty
    pub fn emergency_withdraw(env: Env, staker: Address) -> i128 {
        staker.require_auth();

        let config: StakingConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let staker_info: StakerInfo = env.storage().persistent()
            .get(&DataKey::StakerInfo(staker.clone()))
            .expect("Not staked");

        if staker_info.staked_amount <= 0 {
            panic!("Nothing to withdraw");
        }

        let penalty_amount = (staker_info.staked_amount * config.emergency_penalty as i128) / BASIS_POINTS as i128;
        let amount_to_return = staker_info.staked_amount - penalty_amount;

        // Clear staker info
        let empty_info = StakerInfo {
            staked_amount: 0,
            stake_timestamp: 0,
            last_reward_claim: 0,
            accumulated_rewards: 0,
            tier: StakingTier::None,
        };
        env.storage().persistent().set(&DataKey::StakerInfo(staker.clone()), &empty_info);

        // Transfer tokens back to staker
        let staking_client = token::Client::new(&env, &config.staking_token);
        staking_client.transfer(&env.current_contract_address(), &staker, &amount_to_return);

        // Update total staked
        let total_staked: i128 = env.storage().persistent().get(&DataKey::TotalStaked).unwrap_or(0);
        env.storage().persistent().set(&DataKey::TotalStaked, &(total_staked - staker_info.staked_amount));

        // Remove from stakers list
        Self::remove_from_stakers_list(&env, staker);

        amount_to_return
    }

    // ───────────── VIEW FUNCTIONS ─────────────

    /// Get staker information
    pub fn get_staker_info(env: Env, staker: Address) -> Option<StakerInfo> {
        env.storage().persistent().get(&DataKey::StakerInfo(staker))
    }

    /// Get pending rewards for a staker
    pub fn get_pending_rewards(env: Env, staker: Address) -> i128 {
        let config: StakingConfig = env.storage().persistent().get(&DataKey::Config).unwrap();

        if let Some(staker_info) = Self::get_staker_info(env.clone(), staker) {
            let pending = Self::calculate_pending_rewards(&env, &staker_info, &config);
            staker_info.accumulated_rewards + pending
        } else {
            0
        }
    }

    /// Get total staked amount
    pub fn get_total_staked(env: Env) -> i128 {
        env.storage().persistent().get(&DataKey::TotalStaked).unwrap_or(0)
    }

    /// Get reward pool balance
    pub fn get_reward_pool(env: Env) -> i128 {
        env.storage().persistent().get(&DataKey::RewardPool).unwrap_or(0)
    }

    /// Get staking configuration
    pub fn get_config(env: Env) -> StakingConfig {
        env.storage().persistent().get(&DataKey::Config).unwrap()
    }

    /// Get current APY for a staker (in basis points)
    pub fn get_current_apy(env: Env, staker: Address) -> u32 {
        let config: StakingConfig = env.storage().persistent().get(&DataKey::Config).unwrap();

        if let Some(staker_info) = Self::get_staker_info(env.clone(), staker) {
            Self::get_apy_for_tier(staker_info.tier, &config)
        } else {
            config.base_apy
        }
    }

    /// Get time remaining until lock period ends (0 if already unlocked)
    pub fn get_time_until_unlock(env: Env, staker: Address) -> u64 {
        let config: StakingConfig = env.storage().persistent().get(&DataKey::Config).unwrap();

        if let Some(staker_info) = Self::get_staker_info(env.clone(), staker) {
            let unlock_time = staker_info.stake_timestamp + config.min_lock_period;
            let current_time = env.ledger().timestamp();

            if current_time >= unlock_time {
                0
            } else {
                unlock_time - current_time
            }
        } else {
            0
        }
    }

    /// Check if staker can unstake without penalty
    pub fn can_unstake_without_penalty(env: Env, staker: Address) -> bool {
        Self::get_time_until_unlock(env, staker) == 0
    }

    /// Get all stakers
    pub fn get_all_stakers(env: Env) -> Vec<Address> {
        env.storage().persistent().get(&DataKey::StakersList).unwrap_or(Vec::new(&env))
    }

    // ───────────── INTERNAL HELPERS ─────────────

    fn calculate_pending_rewards(env: &Env, staker_info: &StakerInfo, config: &StakingConfig) -> i128 {
        if staker_info.staked_amount <= 0 {
            return 0;
        }

        let time_elapsed = env.ledger().timestamp() - staker_info.last_reward_claim;
        let apy = Self::get_apy_for_tier(staker_info.tier, config) as i128;

        // Use i128 arithmetic to avoid overflow
        // rewards = staked_amount * apy * time_elapsed / (SECONDS_PER_YEAR * BASIS_POINTS)
        // Split the calculation to avoid overflow: first divide by SECONDS_PER_YEAR, then multiply
        let staked = staker_info.staked_amount;
        let time = time_elapsed as i128;
        let seconds_per_year = SECONDS_PER_YEAR as i128;
        let basis_points = BASIS_POINTS as i128;

        // Calculate: (staked * apy / BASIS_POINTS) * time / SECONDS_PER_YEAR
        // This avoids overflow by doing division earlier
        let annual_reward = (staked * apy) / basis_points;
        let rewards = (annual_reward * time) / seconds_per_year;

        rewards
    }

    fn calculate_tier(staked_amount: i128, config: &StakingConfig) -> StakingTier {
        if staked_amount >= config.gold_threshold {
            StakingTier::Gold
        } else if staked_amount >= config.silver_threshold {
            StakingTier::Silver
        } else if staked_amount >= config.bronze_threshold {
            StakingTier::Bronze
        } else {
            StakingTier::None
        }
    }

    fn get_apy_for_tier(tier: StakingTier, config: &StakingConfig) -> u32 {
        match tier {
            StakingTier::None => config.base_apy,
            StakingTier::Bronze => config.base_apy + config.bronze_bonus,
            StakingTier::Silver => config.base_apy + config.silver_bonus,
            StakingTier::Gold => config.base_apy + config.gold_bonus,
        }
    }

    fn add_to_stakers_list(env: &Env, staker: Address) {
        let mut stakers: Vec<Address> = env.storage().persistent()
            .get(&DataKey::StakersList)
            .unwrap_or(Vec::new(env));

        if !stakers.contains(&staker) {
            stakers.push_back(staker);
            env.storage().persistent().set(&DataKey::StakersList, &stakers);
        }
    }

    fn remove_from_stakers_list(env: &Env, staker: Address) {
        let stakers: Vec<Address> = env.storage().persistent()
            .get(&DataKey::StakersList)
            .unwrap_or(Vec::new(env));

        let mut new_stakers: Vec<Address> = Vec::new(env);
        for s in stakers.iter() {
            if s != staker {
                new_stakers.push_back(s);
            }
        }

        env.storage().persistent().set(&DataKey::StakersList, &new_stakers);
    }

    fn assert_admin(env: &Env, user: &Address) {
        let config: StakingConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        if config.admin != *user {
            panic!("Admin only");
        }
    }

    fn assert_not_paused(env: &Env) {
        let config: StakingConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        if config.paused {
            panic!("Contract is paused");
        }
    }
}

mod test;
