#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, token, Address, Env, Vec, Symbol};

//
// ──────────────────────────────────────────────────────────
// EVENTS
// ──────────────────────────────────────────────────────────
//

#[contracttype]
#[derive(Clone, Debug)]
pub enum Event {
    Staked(Address, i128),
    Unstaked(Address, i128),
    EpochClosed(u64),
    RewardClaimed(Address, u64, i128),
}

//
// ──────────────────────────────────────────────────────────
// DATA KEYS
// ──────────────────────────────────────────────────────────
//

#[contracttype]
pub enum DataKey {
    Config,                           // PoolConfig
    StakePosition(Address),            // StakePosition
    Epoch(u64),                       // Epoch
    PlayerSolves(Address, u64),       // PlayerSolves for epoch
    ClaimedReward(Address, u64),      // bool - if player claimed reward for epoch
    TotalStaked,                      // i128
    CurrentEpochId,                   // u64
    StakersList,                      // Vec<Address>
}

//
// ──────────────────────────────────────────────────────────
// STRUCTS
// ──────────────────────────────────────────────────────────
//

#[contracttype]
#[derive(Clone, Debug)]
pub struct PoolConfig {
    pub admin: Address,
    pub staking_token: Address,
    pub reward_token: Address,
    pub oracle: Address,
    pub epoch_duration: u64,          // Duration of each epoch in seconds
    pub lock_period: u64,             // Lock period for unstaking (7 days)
    pub staker_yield_share: u32,      // Percentage of epoch rewards for stakers (basis points)
    pub solver_yield_share: u32,      // Percentage of epoch rewards for solvers (basis points)
    pub paused: bool,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct StakePosition {
    pub staker: Address,
    pub amount: i128,
    pub staked_at: u64,
    pub last_claim_epoch: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Epoch {
    pub epoch_id: u64,
    pub start_at: u64,
    pub end_at: u64,
    pub total_solves: i128,           // Weighted total solves
    pub total_staked: i128,           // Total staked during epoch
    pub reward_budget: i128,          // Total reward budget for epoch
    pub distributed: bool,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PlayerSolves {
    pub player: Address,
    pub weighted_solves: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct StakeInfo {
    pub position: StakePosition,
    pub unclaimed_epochs: Vec<u64>,
}

//
// ──────────────────────────────────────────────────────────
// CONSTANTS
// ──────────────────────────────────────────────────────────
//

const BASIS_POINTS: u32 = 10_000;
const LOCK_PERIOD_SECONDS: u64 = 7 * 24 * 60 * 60; // 7 days

//
// ──────────────────────────────────────────────────────────
// CONTRACT
// ──────────────────────────────────────────────────────────
//

#[contract]
pub struct PuzzlePoolStaking;

#[contractimpl]
impl PuzzlePoolStaking {
    // ───────────── INITIALIZATION ─────────────

    /// Initialize the puzzle pool staking contract
    pub fn initialize(
        env: Env,
        admin: Address,
        staking_token: Address,
        reward_token: Address,
        oracle: Address,
        epoch_duration: u64,
    ) {
        admin.require_auth();

        if env.storage().persistent().has(&DataKey::Config) {
            panic!("Already initialized");
        }

        let config = PoolConfig {
            admin,
            staking_token,
            reward_token,
            oracle,
            epoch_duration,
            lock_period: LOCK_PERIOD_SECONDS,
            staker_yield_share: 3000,    // 30% for stakers
            solver_yield_share: 7000,    // 70% for solvers
            paused: false,
        };

        env.storage().persistent().set(&DataKey::Config, &config);
        env.storage().persistent().set(&DataKey::TotalStaked, &0i128);
        env.storage().persistent().set(&DataKey::CurrentEpochId, &0u64);
        
        // Create first epoch
        let start_time = env.ledger().timestamp();
        Self::create_epoch(&env, 0, start_time, start_time + epoch_duration);
    }

    // ───────────── ADMIN FUNCTIONS ─────────────

    /// Update yield shares (admin only)
    pub fn update_yield_shares(
        env: Env,
        admin: Address,
        staker_yield_share: u32,
        solver_yield_share: u32,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if staker_yield_share + solver_yield_share != BASIS_POINTS {
            panic!("Yield shares must sum to 10000 basis points");
        }

        let mut config: PoolConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        config.staker_yield_share = staker_yield_share;
        config.solver_yield_share = solver_yield_share;
        env.storage().persistent().set(&DataKey::Config, &config);
    }

    /// Update oracle address (admin only)
    pub fn update_oracle(env: Env, admin: Address, oracle: Address) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut config: PoolConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        config.oracle = oracle;
        env.storage().persistent().set(&DataKey::Config, &config);
    }

    /// Pause/unpause contract (admin only)
    pub fn set_paused(env: Env, admin: Address, paused: bool) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut config: PoolConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        config.paused = paused;
        env.storage().persistent().set(&DataKey::Config, &config);
    }

    /// Add rewards to pool (admin only)
    pub fn add_rewards(env: Env, admin: Address, amount: i128) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let config: PoolConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let reward_client = token::Client::new(&env, &config.reward_token);

        reward_client.transfer(&admin, &env.current_contract_address(), &amount);
    }

    // ───────────── STAKING FUNCTIONS ─────────────

    /// Stake tokens into the pool
    pub fn stake(env: Env, staker: Address, amount: i128) {
        staker.require_auth();
        Self::assert_not_paused(&env);

        if amount <= 0 {
            panic!("Amount must be positive");
        }

        let config: PoolConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let staking_client = token::Client::new(&env, &config.staking_token);

        // Transfer tokens from staker to contract
        staking_client.transfer(&staker, &env.current_contract_address(), &amount);

        // Get or create stake position
        let mut position = Self::get_stake_position(&env, staker.clone())
            .unwrap_or(StakePosition {
                staker: staker.clone(),
                amount: 0,
                staked_at: env.ledger().timestamp(),
                last_claim_epoch: 0,
            });

        // Update position
        position.amount += amount;
        position.staked_at = env.ledger().timestamp();

        env.storage().persistent().set(&DataKey::StakePosition(staker.clone()), &position);

        // Update total staked
        let total_staked: i128 = env.storage().persistent().get(&DataKey::TotalStaked).unwrap_or(0);
        env.storage().persistent().set(&DataKey::TotalStaked, &(total_staked + amount));

        // Add to stakers list
        Self::add_to_stakers_list(&env, staker.clone());

        // Emit event
        env.events().publish((symbol_short!("staked"), staker.clone()), amount);
    }

    /// Unstake tokens (after 7-day lock period)
    pub fn unstake(env: Env, staker: Address, amount: i128) {
        staker.require_auth();
        Self::assert_not_paused(&env);

        if amount <= 0 {
            panic!("Amount must be positive");
        }

        let config: PoolConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let mut position: StakePosition = env.storage().persistent()
            .get(&DataKey::StakePosition(staker.clone()))
            .expect("No stake position found");

        if position.amount < amount {
            panic!("Insufficient staked amount");
        }

        // Check lock period
        let time_staked = env.ledger().timestamp() - position.staked_at;
        if time_staked < config.lock_period {
            panic!("Stake is still locked");
        }

        // Update position
        position.amount -= amount;
        env.storage().persistent().set(&DataKey::StakePosition(staker.clone()), &position);

        // Update total staked
        let total_staked: i128 = env.storage().persistent().get(&DataKey::TotalStaked).unwrap_or(0);
        env.storage().persistent().set(&DataKey::TotalStaked, &(total_staked - amount));

        // Transfer tokens back to staker
        let staking_client = token::Client::new(&env, &config.staking_token);
        staking_client.transfer(&env.current_contract_address(), &staker, &amount);

        // Remove from stakers list if fully unstaked
        if position.amount == 0 {
            Self::remove_from_stakers_list(&env, staker.clone());
        }

        // Emit event
        env.events().publish((symbol_short!("unstaked"), staker.clone()), amount);
    }

    // ───────────── SOLVE RECORDING ─────────────

    /// Record a puzzle solve (oracle only)
    pub fn record_solve(env: Env, oracle: Address, player: Address, puzzle_difficulty: u32) {
        oracle.require_auth();
        Self::assert_oracle(&env, &oracle);
        Self::assert_not_paused(&env);

        if puzzle_difficulty == 0 {
            panic!("Puzzle difficulty must be positive");
        }

        let current_epoch_id: u64 = env.storage().persistent().get(&DataKey::CurrentEpochId).unwrap();
        let mut epoch: Epoch = env.storage().persistent().get(&DataKey::Epoch(current_epoch_id)).unwrap();

        // Check if epoch is still active
        if env.ledger().timestamp() > epoch.end_at {
            panic!("Epoch has ended");
        }

        // Get or create player solves for this epoch
        let mut player_solves = Self::get_player_solves(env.clone(), player.clone(), current_epoch_id)
            .unwrap_or(PlayerSolves {
                player: player.clone(),
                weighted_solves: 0,
            });

        // Add weighted solve (difficulty as weight)
        player_solves.weighted_solves += puzzle_difficulty as i128;
        env.storage().persistent().set(
            &DataKey::PlayerSolves(player.clone(), current_epoch_id),
            &player_solves,
        );

        // Update epoch total solves
        epoch.total_solves += puzzle_difficulty as i128;
        env.storage().persistent().set(&DataKey::Epoch(current_epoch_id), &epoch);
    }

    // ───────────── EPOCH MANAGEMENT ─────────────

    /// Close current epoch and start new one (admin only)
    pub fn close_epoch(env: Env, admin: Address, reward_budget: i128) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        Self::assert_not_paused(&env);

        let config: PoolConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let current_epoch_id: u64 = env.storage().persistent().get(&DataKey::CurrentEpochId).unwrap();
        let mut epoch: Epoch = env.storage().persistent().get(&DataKey::Epoch(current_epoch_id)).unwrap();

        // Check if epoch can be closed
        if env.ledger().timestamp() < epoch.end_at {
            panic!("Epoch has not ended yet");
        }

        if epoch.distributed {
            panic!("Epoch already closed");
        }

        // Set reward budget
        epoch.reward_budget = reward_budget;
        epoch.total_staked = env.storage().persistent().get(&DataKey::TotalStaked).unwrap_or(0);
        epoch.distributed = true;
        env.storage().persistent().set(&DataKey::Epoch(current_epoch_id), &epoch);
        env.storage().persistent().set(&DataKey::CurrentEpochId, &(current_epoch_id + 1));

        env.events().publish((Symbol::new(&env, "ep_close"),), current_epoch_id);

        // Create new epoch
        let new_epoch_id = current_epoch_id + 1;
        let start_time = epoch.end_at;
        let end_time = start_time + config.epoch_duration;
        Self::create_epoch(&env, new_epoch_id, start_time, end_time);
        env.storage().persistent().set(&DataKey::CurrentEpochId, &new_epoch_id);
    }

    // ───────────── REWARD CLAIMING ─────────────

    /// Claim epoch reward for a specific epoch
    pub fn claim_epoch_reward(env: Env, player: Address, epoch_id: u64) -> i128 {
        player.require_auth();
        Self::assert_not_paused(&env);

        // Check if already claimed
        if env.storage().persistent().has(&DataKey::ClaimedReward(player.clone(), epoch_id)) {
            panic!("Reward already claimed for this epoch");
        }

        let epoch: Epoch = env.storage().persistent().get(&DataKey::Epoch(epoch_id))
            .expect("Epoch not found");

        if !epoch.distributed {
            panic!("Epoch rewards not yet distributed");
        }

        let config: PoolConfig = env.storage().persistent().get(&DataKey::Config).unwrap();

        // Calculate reward share
        let reward = Self::calculate_player_reward(&env, &player, epoch_id, &epoch, &config);

        if reward <= 0 {
            panic!("No reward to claim");
        }

        // Mark as claimed
        env.storage().persistent().set(&DataKey::ClaimedReward(player.clone(), epoch_id), &true);

        // Update stake position's last_claim_epoch
        if let Some(mut position) = Self::get_stake_position(&env, player.clone()) {
            if epoch_id > position.last_claim_epoch {
                position.last_claim_epoch = epoch_id;
                env.storage().persistent().set(&DataKey::StakePosition(player.clone()), &position);
            }
        }

        // Transfer reward tokens
        let reward_client = token::Client::new(&env, &config.reward_token);
        reward_client.transfer(&env.current_contract_address(), &player, &reward);

        // Emit event
        env.events().publish((Symbol::new(&env, "rwd_claim"), player.clone()), (epoch_id, reward));

        reward
    }

    // ───────────── VIEW FUNCTIONS ─────────────

    /// Get stake information for a player
    pub fn get_stake(env: Env, player: Address) -> StakeInfo {
        let position = Self::get_stake_position(&env, player.clone())
            .unwrap_or(StakePosition {
                staker: player.clone(),
                amount: 0,
                staked_at: 0,
                last_claim_epoch: 0,
            });

        // Find unclaimed epochs
        let mut unclaimed_epochs = Vec::new(&env);
        let current_epoch_id: u64 = env.storage().persistent().get(&DataKey::CurrentEpochId).unwrap_or(0);

        // Check all epochs from last_claim_epoch + 1 up to current_epoch_id - 1
        // (current_epoch_id is the active epoch, not yet distributed)
        // Special case: if last_claim_epoch is 0, start from 0 (initial state)
        let start_epoch = if position.last_claim_epoch == 0 { 0 } else { position.last_claim_epoch + 1 };
        
        if start_epoch < current_epoch_id {
            for epoch_id in start_epoch..current_epoch_id {
                if let Some(epoch) = env.storage().persistent().get::<DataKey, Epoch>(&DataKey::Epoch(epoch_id)) {
                    if epoch.distributed && !env.storage().persistent().has(&DataKey::ClaimedReward(player.clone(), epoch_id)) {
                        unclaimed_epochs.push_back(epoch_id);
                    }
                }
            }
        }

        StakeInfo {
            position,
            unclaimed_epochs,
        }
    }

    /// Get epoch information
    pub fn get_epoch(env: Env, epoch_id: u64) -> Epoch {
        env.storage().persistent().get(&DataKey::Epoch(epoch_id))
            .expect("Epoch not found")
    }

    /// Get current epoch ID
    pub fn get_current_epoch_id(env: Env) -> u64 {
        env.storage().persistent().get(&DataKey::CurrentEpochId).unwrap_or(0)
    }

    /// Get total staked amount
    pub fn get_total_staked(env: Env) -> i128 {
        env.storage().persistent().get(&DataKey::TotalStaked).unwrap_or(0)
    }

    /// Get player solves for an epoch
    pub fn get_player_solves(env: Env, player: Address, epoch_id: u64) -> Option<PlayerSolves> {
        env.storage().persistent().get(&DataKey::PlayerSolves(player, epoch_id))
    }

    /// Check if reward claimed for epoch
    pub fn is_reward_claimed(env: Env, player: Address, epoch_id: u64) -> bool {
        env.storage().persistent().get(&DataKey::ClaimedReward(player, epoch_id)).unwrap_or(false)
    }

    /// Get configuration
    pub fn get_config(env: Env) -> PoolConfig {
        env.storage().persistent().get(&DataKey::Config).unwrap()
    }

    /// Get all stakers
    pub fn get_all_stakers(env: Env) -> Vec<Address> {
        env.storage().persistent().get(&DataKey::StakersList).unwrap_or(Vec::new(&env))
    }

    // ───────────── INTERNAL HELPERS ─────────────

    fn create_epoch(env: &Env, epoch_id: u64, start_at: u64, end_at: u64) {
        let epoch = Epoch {
            epoch_id,
            start_at,
            end_at,
            total_solves: 0,
            total_staked: 0,
            reward_budget: 0,
            distributed: false,
        };
        env.storage().persistent().set(&DataKey::Epoch(epoch_id), &epoch);
    }

    fn get_stake_position(env: &Env, staker: Address) -> Option<StakePosition> {
        env.storage().persistent().get(&DataKey::StakePosition(staker))
    }

    fn calculate_player_reward(
        env: &Env,
        player: &Address,
        epoch_id: u64,
        epoch: &Epoch,
        config: &PoolConfig,
    ) -> i128 {
        let mut total_reward = 0i128;

        // Calculate solver reward share
        if let Some(player_solves) = Self::get_player_solves(env.clone(), player.clone(), epoch_id) {
            if epoch.total_solves > 0 && player_solves.weighted_solves > 0 {
                let solver_share = (epoch.reward_budget * config.solver_yield_share as i128) / BASIS_POINTS as i128;
                let solver_reward = (solver_share * player_solves.weighted_solves) / epoch.total_solves;
                total_reward += solver_reward;
            }
        }

        // Calculate staker reward share
        if let Some(position) = Self::get_stake_position(env, player.clone()) {
            if position.amount > 0 && epoch.total_staked > 0 {
                let staker_share = (epoch.reward_budget * config.staker_yield_share as i128) / BASIS_POINTS as i128;
                let staker_reward = (staker_share * position.amount) / epoch.total_staked;
                total_reward += staker_reward;
            }
        }

        total_reward
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
        let config: PoolConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        if config.admin != *user {
            panic!("Admin only");
        }
    }

    fn assert_oracle(env: &Env, user: &Address) {
        let config: PoolConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        if config.oracle != *user {
            panic!("Oracle only");
        }
    }

    fn assert_not_paused(env: &Env) {
        let config: PoolConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        if config.paused {
            panic!("Contract is paused");
        }
    }
}

mod test;
