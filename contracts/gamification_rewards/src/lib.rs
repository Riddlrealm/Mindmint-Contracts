#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec, Map};

// ──────────────────────────────────────────────────────────
// CONSTANTS
// ──────────────────────────────────────────────────────────

#[cfg(not(test))]
const DAY_IN_LEDGERS: u32 = 17280; // ≈ 24 hours (5s per ledger)
#[cfg(test)]
const DAY_IN_LEDGERS: u32 = 2;

#[cfg(not(test))]
const WEEK_IN_LEDGERS: u32 = 120960; // 7 days
#[cfg(test)]
const WEEK_IN_LEDGERS: u32 = 14;

const MAX_COMBO_CHAIN: u32 = 100;
const MAX_STREAK_MULTIPLIER: u32 = 500; // 5x max
const BASE_MULTIPLIER: u32 = 100; // 1x base (100 = 1.0x)

// ──────────────────────────────────────────────────────────
// DATA STRUCTURES
// ──────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiplierType {
    Streak = 0,
    Combo = 1,
    Milestone = 2,
    Boost = 3,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreakType {
    Daily = 0,
    Weekly = 1,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MultiplierState {
    pub base_multiplier: u32,         // Base multiplier (100 = 1x)
    pub streak_multiplier: u32,       // Streak bonus
    pub combo_multiplier: u32,        // Combo chain bonus
    pub milestone_multiplier: u32,    // Milestone unlocks
    pub boost_multiplier: u32,        // Temporary boosts
    pub total_multiplier: u32,        // Final calculated multiplier
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct StreakData {
    pub daily_streak: u32,
    pub weekly_streak: u32,
    pub last_daily_claim: u32,
    pub last_weekly_claim: u32,
    pub best_daily_streak: u32,
    pub best_weekly_streak: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ComboChain {
    pub current_combo: u32,
    pub best_combo: u32,
    pub last_action_ledger: u32,
    pub combo_decay_start: u32, // Ledger when decay starts
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MilestoneProgress {
    pub total_actions: u32,
    pub milestones_unlocked: u32,
    pub permanent_bonus: u32, // Permanent multiplier bonus
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct BoostItem {
    pub boost_type: BoostType,
    pub multiplier_bonus: u32,
    pub start_ledger: u32,
    pub duration_ledgers: u32,
    pub is_active: bool,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoostType {
    SpeedBoost = 0,      // Short duration, high multiplier
    LuckBoost = 1,       // Medium duration, medium multiplier
    PowerBoost = 2,      // Long duration, low multiplier
    SuperBoost = 3,      // Very short, very high multiplier
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MultiplierHistoryEntry {
    pub ledger: u32,
    pub multiplier: u32,
    pub breakdown: MultiplierState,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PlayerLeaderboardEntry {
    pub player: Address,
    pub total_multiplier: u32,
    pub total_actions: u32,
    pub best_combo: u32,
    pub best_streak: u32,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Config,
    Admin,
    PlayerMultiplier(Address),           // MultiplierState
    PlayerStreak(Address),               // StreakData
    PlayerCombo(Address),                // ComboChain
    PlayerMilestone(Address),            // MilestoneProgress
    PlayerBoosts(Address),               // Vec<BoostItem>
    PlayerHistory(Address),              // Vec<MultiplierHistoryEntry>
    Leaderboard,                         // Vec<PlayerLeaderboardEntry>
    GlobalStats,                         // GlobalStats
    Verifier(Address),                   // bool
    MilestoneThreshold(u32),             // u32 threshold for milestone level
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct GlobalStats {
    pub total_players: u32,
    pub total_multipliers_applied: u32,
    pub highest_combo: u32,
    pub highest_streak: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Config {
    pub admin: Address,
    pub combo_decay_period: u32,     // Ledgers before combo starts decaying
    pub combo_decay_rate: u32,       // How much combo decays per ledger
    pub max_leaderboard_entries: u32,
    pub paused: bool,
}

// ──────────────────────────────────────────────────────────
// EVENTS
// ──────────────────────────────────────────────────────────

const MULTIPLIER_UPDATE: Symbol = symbol_short!("mult_up");
const STREAK_ACHIEVED: Symbol = symbol_short!("streak");
const COMBO_RECORD: Symbol = symbol_short!("combo");
const MILESTONE_UNLOCK: Symbol = symbol_short!("unlock");
const BOOST_ACTIVATED: Symbol = symbol_short!("boost");
const BOOST_EXPIRED: Symbol = symbol_short!("boost_x");

// ──────────────────────────────────────────────────────────
// CONTRACT
// ──────────────────────────────────────────────────────────

#[contract]
pub struct GamificationRewardsContract;

#[contractimpl]
impl GamificationRewardsContract {
    // ───────────── INITIALIZATION ─────────────

    /// Initialize the gamification rewards contract
    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();

        if env.storage().persistent().has(&DataKey::Config) {
            panic!("Already initialized");
        }

        let config = Config {
            admin,
            combo_decay_period: DAY_IN_LEDGERS, // Combo starts decaying after 1 day of inactivity
            combo_decay_rate: 1,                // Lose 1 combo point per ledger after decay starts
            max_leaderboard_entries: 100,
            paused: false,
        };

        env.storage().persistent().set(&DataKey::Config, &config);
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::GlobalStats, &GlobalStats {
                total_players: 0,
                total_multipliers_applied: 0,
                highest_combo: 0,
                highest_streak: 0,
            });

        // Set default milestone thresholds
        Self::set_milestone_threshold(&env, 1, 10);    // Level 1: 10 actions
        Self::set_milestone_threshold(&env, 2, 50);    // Level 2: 50 actions
        Self::set_milestone_threshold(&env, 3, 100);   // Level 3: 100 actions
        Self::set_milestone_threshold(&env, 4, 250);   // Level 4: 250 actions
        Self::set_milestone_threshold(&env, 5, 500);   // Level 5: 500 actions
        Self::set_milestone_threshold(&env, 6, 1000);  // Level 6: 1000 actions
    }

    // ───────────── ADMIN FUNCTIONS ─────────────

    /// Add an authorized score verifier
    pub fn add_verifier(env: Env, admin: Address, verifier: Address) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::Verifier(verifier), &true);
    }

    /// Remove an authorized verifier
    pub fn remove_verifier(env: Env, admin: Address, verifier: Address) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        env.storage().persistent().remove(&DataKey::Verifier(verifier));
    }

    /// Pause/unpause the contract
    pub fn set_paused(env: Env, admin: Address, paused: bool) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        let mut config: Config = env.storage().persistent().get(&DataKey::Config).unwrap();
        config.paused = paused;
        env.storage().persistent().set(&DataKey::Config, &config);
    }

    /// Update combo decay parameters
    pub fn update_combo_decay(
        env: Env,
        admin: Address,
        combo_decay_period: u32,
        combo_decay_rate: u32,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        let mut config: Config = env.storage().persistent().get(&DataKey::Config).unwrap();
        config.combo_decay_period = combo_decay_period;
        config.combo_decay_rate = combo_decay_rate;
        env.storage().persistent().set(&DataKey::Config, &config);
    }

    /// Set milestone threshold for a specific level
    pub fn set_milestone_threshold(env: Env, admin: Address, level: u32, threshold: u32) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::MilestoneThreshold(level), &threshold);
    }

    // ───────────── STREAK FUNCTIONS ─────────────

    /// Record a daily action and update streak
    pub fn record_daily_action(env: Env, player: Address) -> u32 {
        player.require_auth();
        Self::assert_not_paused(&env);

        let current_ledger = env.ledger().sequence();
        let mut streak_data: StreakData = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerStreak(player.clone()))
            .unwrap_or(StreakData {
                daily_streak: 0,
                weekly_streak: 0,
                last_daily_claim: 0,
                last_weekly_claim: 0,
                best_daily_streak: 0,
                best_weekly_streak: 0,
            });

        let ledgers_since_last = current_ledger.saturating_sub(streak_data.last_daily_claim);

        // Check if this is a new day
        if ledgers_since_last >= DAY_IN_LEDGERS {
            // Consecutive day
            if ledgers_since_last < (DAY_IN_LEDGERS * 2) {
                streak_data.daily_streak += 1;
                if streak_data.daily_streak > streak_data.best_daily_streak {
                    streak_data.best_daily_streak = streak_data.daily_streak;
                }
            } else {
                // Streak broken
                streak_data.daily_streak = 1;
            }

            streak_data.last_daily_claim = current_ledger;

            // Update weekly streak
            let weeks_since_last = current_ledger.saturating_sub(streak_data.last_weekly_claim);
            if weeks_since_last >= WEEK_IN_LEDGERS {
                if weeks_since_last < (WEEK_IN_LEDGERS * 2) {
                    streak_data.weekly_streak += 1;
                    if streak_data.weekly_streak > streak_data.best_weekly_streak {
                        streak_data.best_weekly_streak = streak_data.weekly_streak;
                    }
                } else {
                    streak_data.weekly_streak = 1;
                }
                streak_data.last_weekly_claim = current_ledger;
            }

            env.storage()
                .persistent()
                .set(&DataKey::PlayerStreak(player.clone()), &streak_data);

            // Emit event
            env.events().publish(
                (STREAK_ACHIEVED, player.clone()),
                (streak_data.daily_streak, streak_data.weekly_streak),
            );
        }

        // Calculate and return streak multiplier
        Self::calculate_streak_multiplier(&streak_data)
    }

    /// Get current streak data for a player
    pub fn get_streak_data(env: Env, player: Address) -> StreakData {
        env.storage()
            .persistent()
            .get(&DataKey::PlayerStreak(player))
            .unwrap_or(StreakData {
                daily_streak: 0,
                weekly_streak: 0,
                last_daily_claim: 0,
                last_weekly_claim: 0,
                best_daily_streak: 0,
                best_weekly_streak: 0,
            })
    }

    // ───────────── COMBO CHAIN FUNCTIONS ─────────────

    /// Record a combo action
    pub fn record_combo_action(env: Env, player: Address) -> u32 {
        player.require_auth();
        Self::assert_not_paused(&env);

        let current_ledger = env.ledger().sequence();
        let config: Config = env.storage().persistent().get(&DataKey::Config).unwrap();

        let mut combo_data: ComboChain = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerCombo(player.clone()))
            .unwrap_or(ComboChain {
                current_combo: 0,
                best_combo: 0,
                last_action_ledger: 0,
                combo_decay_start: 0,
            });

        // Check if combo should decay
        if combo_data.current_combo > 0 {
            let ledgers_since_last = current_ledger.saturating_sub(combo_data.last_action_ledger);
            
            if ledgers_since_last > config.combo_decay_period {
                // Start decaying combo
                let decay_ledgers = ledgers_since_last - config.combo_decay_period;
                let decay_amount = decay_ledgers * config.combo_decay_rate;
                combo_data.current_combo = combo_data.current_combo.saturating_sub(decay_amount);
            }
        }

        // Increment combo
        combo_data.current_combo = (combo_data.current_combo + 1).min(MAX_COMBO_CHAIN);
        
        if combo_data.current_combo > combo_data.best_combo {
            combo_data.best_combo = combo_data.current_combo;
            
            // Update global stats
            Self::update_global_combo_record(&env, combo_data.best_combo);
        }

        combo_data.last_action_ledger = current_ledger;
        combo_data.combo_decay_start = current_ledger + config.combo_decay_period;

        env.storage()
            .persistent()
            .set(&DataKey::PlayerCombo(player.clone()), &combo_data);

        // Emit event
        env.events().publish(
            (COMBO_RECORD, player.clone()),
            combo_data.current_combo,
        );

        // Calculate and return combo multiplier
        Self::calculate_combo_multiplier(combo_data.current_combo)
    }

    /// Get current combo data for a player
    pub fn get_combo_data(env: Env, player: Address) -> ComboChain {
        env.storage()
            .persistent()
            .get(&DataKey::PlayerCombo(player))
            .unwrap_or(ComboChain {
                current_combo: 0,
                best_combo: 0,
                last_action_ledger: 0,
                combo_decay_start: 0,
            })
    }

    // ───────────── MILESTONE FUNCTIONS ─────────────

    /// Record a milestone action
    pub fn record_milestone_action(env: Env, player: Address) -> u32 {
        let admin = Self::get_admin(&env);
        admin.require_auth();
        Self::assert_not_paused(&env);

        let mut milestone_data: MilestoneProgress = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerMilestone(player.clone()))
            .unwrap_or(MilestoneProgress {
                total_actions: 0,
                milestones_unlocked: 0,
                permanent_bonus: 0,
            });

        milestone_data.total_actions += 1;

        // Check for milestone unlocks
        let mut new_milestones = 0;
        for level in 1..=10 {
            let threshold_key = DataKey::MilestoneThreshold(level);
            if let Some(threshold) = env.storage().persistent().get(&threshold_key) {
                if milestone_data.total_actions >= threshold && level > milestone_data.milestones_unlocked {
                    new_milestones += 1;
                    milestone_data.milestones_unlocked = level;
                    
                    // Grant permanent bonus (5% per milestone level)
                    milestone_data.permanent_bonus += 5;
                    
                    // Emit event
                    env.events().publish(
                        (MILESTONE_UNLOCK, player.clone()),
                        (level, milestone_data.permanent_bonus),
                    );
                }
            }
        }

        env.storage()
            .persistent()
            .set(&DataKey::PlayerMilestone(player.clone()), &milestone_data);

        // Calculate and return milestone multiplier
        Self::calculate_milestone_multiplier(milestone_data.permanent_bonus)
    }

    /// Get milestone progress for a player
    pub fn get_milestone_progress(env: Env, player: Address) -> MilestoneProgress {
        env.storage()
            .persistent()
            .get(&DataKey::PlayerMilestone(player))
            .unwrap_or(MilestoneProgress {
                total_actions: 0,
                milestones_unlocked: 0,
                permanent_bonus: 0,
            })
    }

    // ───────────── BOOST ITEM FUNCTIONS ─────────────

    /// Activate a boost item for a player
    pub fn activate_boost(
        env: Env,
        admin: Address,
        player: Address,
        boost_type: BoostType,
        duration_ledgers: u32,
    ) {
        admin.require_auth();
        Self::assert_not_paused(&env);

        let multiplier_bonus = match boost_type {
            BoostType::SpeedBoost => 50,    // 0.5x bonus
            BoostType::LuckBoost => 30,     // 0.3x bonus
            BoostType::PowerBoost => 20,    // 0.2x bonus
            BoostType::SuperBoost => 100,   // 1.0x bonus
        };

        let current_ledger = env.ledger().sequence();
        let boost_item = BoostItem {
            boost_type,
            multiplier_bonus,
            start_ledger: current_ledger,
            duration_ledgers,
            is_active: true,
        };

        // Get existing boosts or create new vector
        let mut boosts: Vec<BoostItem> = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerBoosts(player.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        boosts.push_back(boost_item);
        env.storage()
            .persistent()
            .set(&DataKey::PlayerBoosts(player.clone()), &boosts);

        // Emit event
        env.events().publish(
            (BOOST_ACTIVATED, player.clone()),
            (boost_type, multiplier_bonus, duration_ledgers),
        );
    }

    /// Deactivate a specific boost
    pub fn deactivate_boost(env: Env, admin: Address, player: Address, boost_index: u32) {
        admin.require_auth();

        let mut boosts: Vec<BoostItem> = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerBoosts(player.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        if let Some(boost) = boosts.get(boost_index) {
            let mut updated_boost = boost.clone();
            updated_boost.is_active = false;
            boosts.set(boost_index, &updated_boost);

            env.storage()
                .persistent()
                .set(&DataKey::PlayerBoosts(player.clone()), &boosts);
        }
    }

    /// Get active boosts for a player
    pub fn get_active_boosts(env: Env, player: Address) -> Vec<BoostItem> {
        let boosts: Vec<BoostItem> = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerBoosts(player.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        let current_ledger = env.ledger().sequence();
        let mut active_boosts = Vec::new(&env);

        for i in 0..boosts.len() {
            if let Some(boost) = boosts.get(i) {
                if boost.is_active && current_ledger < (boost.start_ledger + boost.duration_ledgers) {
                    active_boosts.push_back(boost);
                }
            }
        }

        active_boosts
    }

    // ───────────── MULTIPLIER CALCULATION ─────────────

    /// Get the total multiplier for a player
    pub fn get_total_multiplier(env: Env, player: Address) -> MultiplierState {
        let streak_data = Self::get_streak_data(env.clone(), player.clone());
        let combo_data = Self::get_combo_data(env.clone(), player.clone());
        let milestone_data = Self::get_milestone_progress(env.clone(), player.clone());
        let boosts = Self::get_active_boosts(env.clone(), player.clone());

        let streak_mult = Self::calculate_streak_multiplier(&streak_data);
        let combo_mult = Self::calculate_combo_multiplier(combo_data.current_combo);
        let milestone_mult = Self::calculate_milestone_multiplier(milestone_data.permanent_bonus);
        let boost_mult = Self::calculate_boost_multiplier(&boosts);

        // Apply stacking rules: additive within categories, multiplicative across
        let total_mult = Self::apply_stacking_rules(
            streak_mult,
            combo_mult,
            milestone_mult,
            boost_mult,
        );

        let state = MultiplierState {
            base_multiplier: BASE_MULTIPLIER,
            streak_multiplier: streak_mult,
            combo_multiplier: combo_mult,
            milestone_multiplier: milestone_mult,
            boost_multiplier: boost_mult,
            total_multiplier: total_mult,
        };

        // Record history
        Self::record_multiplier_history(&env, &player, &state);

        state
    }

    /// Calculate final reward with multipliers applied
    pub fn calculate_reward(env: Env, player: Address, base_reward: u128) -> u128 {
        let multiplier_state = Self::get_total_multiplier(env.clone(), player.clone());
        
        // Apply multiplier: base_reward * (total_multiplier / 100)
        let multiplied_reward = (base_reward * multiplier_state.total_multiplier as u128) / 100;

        // Update global stats
        Self::increment_multipliers_applied(&env);

        multiplied_reward
    }

    // ───────────── HISTORY TRACKING ─────────────

    /// Get multiplier history for a player
    pub fn get_multiplier_history(
        env: Env,
        player: Address,
        limit: u32,
    ) -> Vec<MultiplierHistoryEntry> {
        let history: Vec<MultiplierHistoryEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerHistory(player))
            .unwrap_or_else(|| Vec::new(&env));

        let actual_limit = if limit > history.len() {
            history.len()
        } else {
            limit
        };

        let mut result = Vec::new(&env);
        for i in 0..actual_limit {
            if let Some(entry) = history.get(history.len() - 1 - i) {
                result.push_back(entry);
            }
        }

        result
    }

    // ───────────── LEADERBOARD ─────────────

    /// Update leaderboard with player's current stats
    pub fn update_leaderboard(env: Env, player: Address) {
        Self::assert_not_paused(&env);

        let multiplier_state = Self::get_total_multiplier(env.clone(), player.clone());
        let streak_data = Self::get_streak_data(env.clone(), player.clone());
        let combo_data = Self::get_combo_data(env.clone(), player.clone());
        let milestone_data = Self::get_milestone_progress(env.clone(), player.clone());

        let entry = PlayerLeaderboardEntry {
            player: player.clone(),
            total_multiplier: multiplier_state.total_multiplier,
            total_actions: milestone_data.total_actions,
            best_combo: combo_data.best_combo,
            best_streak: streak_data.best_daily_streak.max(streak_data.best_weekly_streak),
        };

        // Get current leaderboard
        let mut leaderboard: Vec<PlayerLeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::Leaderboard)
            .unwrap_or_else(|| Vec::new(&env));

        // Remove existing entry for this player
        let mut new_leaderboard = Vec::new(&env);
        for i in 0..leaderboard.len() {
            if let Some(existing) = leaderboard.get(i) {
                if existing.player != player {
                    new_leaderboard.push_back(existing);
                }
            }
        }

        // Insert new entry in sorted order (by total_multiplier descending)
        let mut inserted = false;
        let mut final_leaderboard = Vec::new(&env);
        let config: Config = env.storage().persistent().get(&DataKey::Config).unwrap();

        for i in 0..new_leaderboard.len() {
            if !inserted {
                if let Some(existing) = new_leaderboard.get(i) {
                    if entry.total_multiplier > existing.total_multiplier {
                        final_leaderboard.push_back(entry.clone());
                        inserted = true;
                    }
                }
            }
            if final_leaderboard.len() < config.max_leaderboard_entries {
                if let Some(existing) = new_leaderboard.get(i) {
                    final_leaderboard.push_back(existing);
                }
            }
        }

        if !inserted && final_leaderboard.len() < config.max_leaderboard_entries {
            final_leaderboard.push_back(entry);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Leaderboard, &final_leaderboard);
    }

    /// Get top players by multiplier
    pub fn get_leaderboard(env: Env, limit: u32) -> Vec<PlayerLeaderboardEntry> {
        let leaderboard: Vec<PlayerLeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::Leaderboard)
            .unwrap_or_else(|| Vec::new(&env));

        let actual_limit = if limit > leaderboard.len() {
            leaderboard.len()
        } else {
            limit
        };

        let mut result = Vec::new(&env);
        for i in 0..actual_limit {
            if let Some(entry) = leaderboard.get(i) {
                result.push_back(entry);
            }
        }

        result
    }

    /// Get player's rank on leaderboard
    pub fn get_player_rank(env: Env, player: Address) -> u32 {
        let leaderboard: Vec<PlayerLeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::Leaderboard)
            .unwrap_or_else(|| Vec::new(&env));

        for i in 0..leaderboard.len() {
            if let Some(entry) = leaderboard.get(i) {
                if entry.player == player {
                    return (i + 1) as u32;
                }
            }
        }

        0 // Not ranked
    }

    // ───────────── VIEW FUNCTIONS ─────────────

    /// Get global statistics
    pub fn get_global_stats(env: Env) -> GlobalStats {
        env.storage()
            .persistent()
            .get(&DataKey::GlobalStats)
            .unwrap_or(GlobalStats {
                total_players: 0,
                total_multipliers_applied: 0,
                highest_combo: 0,
                highest_streak: 0,
            })
    }

    /// Get configuration
    pub fn get_config(env: Env) -> Config {
        env.storage().persistent().get(&DataKey::Config).unwrap()
    }

    // ───────────── INTERNAL HELPERS ─────────────

    fn calculate_streak_multiplier(streak_data: &StreakData) -> u32 {
        let daily_bonus = (streak_data.daily_streak.min(30) * 5).min(150); // Max 1.5x from daily
        let weekly_bonus = (streak_data.weekly_streak.min(12) * 10).min(120); // Max 1.2x from weekly
        
        BASE_MULTIPLIER + daily_bonus + weekly_bonus
    }

    fn calculate_combo_multiplier(combo: u32) -> u32 {
        // 1% bonus per combo point, compounding
        BASE_MULTIPLIER + (combo * 2).min(200) // Max 2x from combo
    }

    fn calculate_milestone_multiplier(permanent_bonus: u32) -> u32 {
        BASE_MULTIPLIER + permanent_bonus
    }

    fn calculate_boost_multiplier(boosts: &Vec<BoostItem>) -> u32 {
        let mut total_boost = 0;
        for i in 0..boosts.len() {
            if let Some(boost) = boosts.get(i) {
                if boost.is_active {
                    total_boost += boost.multiplier_bonus;
                }
            }
        }
        BASE_MULTIPLIER + total_boost
    }

    fn apply_stacking_rules(
        streak_mult: u32,
        combo_mult: u32,
        milestone_mult: u32,
        boost_mult: u32,
    ) -> u32 {
        // Multiplicative stacking across categories
        // To avoid overflow, we use a simplified formula:
        // total = base * (1 + sum_of_bonuses/100)
        
        let streak_bonus = streak_mult - BASE_MULTIPLIER;
        let combo_bonus = combo_mult - BASE_MULTIPLIER;
        let milestone_bonus = milestone_mult - BASE_MULTIPLIER;
        let boost_bonus = boost_mult - BASE_MULTIPLIER;

        let total_bonus = streak_bonus + combo_bonus + milestone_bonus + boost_bonus;
        
        (BASE_MULTIPLIER + (total_bonus / 4)).min(MAX_STREAK_MULTIPLIER)
    }

    fn record_multiplier_history(env: &Env, player: &Address, state: &MultiplierState) {
        let mut history: Vec<MultiplierHistoryEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerHistory(player.clone()))
            .unwrap_or_else(|| Vec::new(env));

        let entry = MultiplierHistoryEntry {
            ledger: env.ledger().sequence(),
            multiplier: state.total_multiplier,
            breakdown: state.clone(),
        };

        // Keep only last 50 entries to save storage
        if history.len() >= 50 {
            history = history.slice(1..history.len());
        }

        history.push_back(entry);
        env.storage()
            .persistent()
            .set(&DataKey::PlayerHistory(player.clone()), &history);
    }

    fn update_global_combo_record(env: &Env, combo: u32) {
        let mut stats: GlobalStats = env
            .storage()
            .persistent()
            .get(&DataKey::GlobalStats)
            .unwrap_or(GlobalStats {
                total_players: 0,
                total_multipliers_applied: 0,
                highest_combo: 0,
                highest_streak: 0,
            });

        if combo > stats.highest_combo {
            stats.highest_combo = combo;
            env.storage().persistent().set(&DataKey::GlobalStats, &stats);
        }
    }

    fn increment_multipliers_applied(env: &Env) {
        let mut stats: GlobalStats = env
            .storage()
            .persistent()
            .get(&DataKey::GlobalStats)
            .unwrap_or(GlobalStats {
                total_players: 0,
                total_multipliers_applied: 0,
                highest_combo: 0,
                highest_streak: 0,
            });

        stats.total_multipliers_applied += 1;
        env.storage().persistent().set(&DataKey::GlobalStats, &stats);
    }

    fn get_admin(env: &Env) -> Address {
        env.storage().persistent().get(&DataKey::Admin).unwrap()
    }

    fn assert_admin(env: &Env, user: &Address) {
        let admin = Self::get_admin(env);
        if admin != *user {
            panic!("Admin only");
        }
    }

    fn assert_not_paused(env: &Env) {
        let config: Config = env.storage().persistent().get(&DataKey::Config).unwrap();
        if config.paused {
            panic!("Contract is paused");
        }
    }
}

// ──────────────────────────────────────────────────────────
// TESTS
// ──────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};

    fn setup() -> (Env, Address, Address, GamificationRewardsContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, GamificationRewardsContract);
        let client = GamificationRewardsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let player = Address::generate(&env);

        (env, admin, player, client)
    }

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let contract_id = env.register_contract(None, GamificationRewardsContract);
        let client = GamificationRewardsContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client.initialize(&admin);
        
        let config = client.get_config();
        assert_eq!(config.admin, admin);
        assert!(!config.paused);
    }

    #[test]
    fn test_daily_streak_increases() {
        let (env, _admin, player, client) = setup();

        // First action
        let mult1 = client.record_daily_action(&player);
        assert_eq!(mult1, 110); // Base 100 + 10 for first streak

        // Advance 1 day
        env.ledger().with_mut(|li| li.sequence_number += DAY_IN_LEDGERS);

        // Second action
        let mult2 = client.record_daily_action(&player);
        assert!(mult2 > mult1); // Should increase

        let streak = client.get_streak_data(&player);
        assert_eq!(streak.daily_streak, 2);
    }

    #[test]
    fn test_combo_chain_stacks() {
        let (_env, _admin, player, client) = setup();

        // First combo
        let mult1 = client.record_combo_action(&player);
        assert_eq!(mult1, 102); // Base 100 + 2 for first combo

        // Second combo
        let mult2 = client.record_combo_action(&player);
        assert_eq!(mult2, 104); // Base 100 + 4 for second combo

        let combo = client.get_combo_data(&player);
        assert_eq!(combo.current_combo, 2);
        assert_eq!(combo.best_combo, 2);
    }

    #[test]
    fn test_milestone_unlocks_permanent_bonus() {
        let (env, admin, player, client) = setup();

        // Set low threshold for testing
        client.set_milestone_threshold(&admin, &1, &5);

        // Record 5 actions
        for _ in 0..5 {
            client.record_milestone_action(&admin, &player);
        }

        let milestone = client.get_milestone_progress(&player);
        assert_eq!(milestone.total_actions, 5);
        assert_eq!(milestone.milestones_unlocked, 1);
        assert_eq!(milestone.permanent_bonus, 5); // 5% bonus
    }

    #[test]
    fn test_boost_item_activation() {
        let (env, admin, player, client) = setup();

        // Activate speed boost for 10 ledgers
        client.activate_boost(&admin, &player, &BoostType::SpeedBoost, &10);

        let boosts = client.get_active_boosts(&player);
        assert_eq!(boosts.len(), 1);
        assert_eq!(boosts.get(0).unwrap().boost_type, BoostType::SpeedBoost);
        assert_eq!(boosts.get(0).unwrap().multiplier_bonus, 50);

        // Advance beyond boost duration
        env.ledger().with_mut(|li| li.sequence_number += 11);

        let expired_boosts = client.get_active_boosts(&player);
        assert_eq!(expired_boosts.len(), 0);
    }

    #[test]
    fn test_multiplier_calculation() {
        let (env, _admin, player, client) = setup();

        // Build up some multipliers
        client.record_daily_action(&player);
        client.record_combo_action(&player);

        let state = client.get_total_multiplier(&player);
        assert!(state.total_multiplier >= BASE_MULTIPLIER);
        assert_eq!(state.base_multiplier, BASE_MULTIPLIER);
    }

    #[test]
    fn test_reward_calculation() {
        let (_env, _admin, player, client) = setup();

        // No bonuses yet
        let base_reward = 1000;
        let reward1 = client.calculate_reward(&player, &base_reward);
        assert_eq!(reward1, 1000); // Base multiplier is 100 (1x)

        // Build up combo
        for _ in 0..5 {
            client.record_combo_action(&player);
        }

        let reward2 = client.calculate_reward(&player, &base_reward);
        assert!(reward2 > reward1); // Should be higher with combo bonus
    }

    #[test]
    fn test_leaderboard_updates() {
        let (_env, _admin, player, client) = setup();

        // Build up player stats
        client.record_daily_action(&player);
        for _ in 0..10 {
            client.record_combo_action(&player);
        }

        client.update_leaderboard(&player);

        let leaderboard = client.get_leaderboard(&10);
        assert_eq!(leaderboard.len(), 1);
        assert_eq!(leaderboard.get(0).unwrap().player, player);

        let rank = client.get_player_rank(&player);
        assert_eq!(rank, 1);
    }

    #[test]
    fn test_multiplier_history_tracking() {
        let (env, _admin, player, client) = setup();

        // Record several actions
        for i in 0..5 {
            client.record_daily_action(&player);
            client.record_combo_action(&player);
            
            if i < 4 {
                env.ledger().with_mut(|li| li.sequence_number += DAY_IN_LEDGERS);
            }
        }

        let history = client.get_multiplier_history(&player, &10);
        assert!(history.len() > 0);
        assert!(history.len() <= 10);
    }

    #[test]
    fn test_combo_decay() {
        let (env, _admin, player, client) = setup();

        // Build combo to 10
        for _ in 0..10 {
            client.record_combo_action(&player);
        }

        let combo1 = client.get_combo_data(&player);
        assert_eq!(combo1.current_combo, 10);

        // Advance beyond decay period
        env.ledger().with_mut(|li| li.sequence_number += DAY_IN_LEDGERS + 5);

        // Next action should trigger decay
        client.record_combo_action(&player);
        
        let combo2 = client.get_combo_data(&player);
        assert!(combo2.current_combo < 10); // Should have decayed
    }

    #[test]
    fn test_global_stats_tracking() {
        let (_env, _admin, player, client) = setup();

        // Build a large combo
        for _ in 0..50 {
            client.record_combo_action(&player);
        }

        let stats = client.get_global_stats();
        assert!(stats.highest_combo >= 50);
        assert!(stats.total_multipliers_applied > 0);
    }
}
