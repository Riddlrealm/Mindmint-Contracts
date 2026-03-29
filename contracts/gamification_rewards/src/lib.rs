#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec};

// (removed unused Map import)

// ─────────────────────────────────────────
// CONSTANTS
// ─────────────────────────────────────────

#[cfg(not(test))]
const DAY_IN_LEDGERS: u32 = 17280;
#[cfg(test)]
const DAY_IN_LEDGERS: u32 = 2;

#[cfg(not(test))]
const WEEK_IN_LEDGERS: u32 = 120960;
#[cfg(test)]
const WEEK_IN_LEDGERS: u32 = 14;

const MAX_COMBO_CHAIN: u32 = 100;
const MAX_STREAK_MULTIPLIER: u32 = 500;
const BASE_MULTIPLIER: u32 = 100;

// ─────────────────────────────────────────
// TYPES
// ─────────────────────────────────────────

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiplierType {
    Streak,
    Combo,
    Milestone,
    Boost,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreakType {
    Daily,
    Weekly,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MultiplierState {
    pub base_multiplier: u32,
    pub streak_multiplier: u32,
    pub combo_multiplier: u32,
    pub milestone_multiplier: u32,
    pub boost_multiplier: u32,
    pub total_multiplier: u32,
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
    pub combo_decay_start: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MilestoneProgress {
    pub total_actions: u32,
    pub milestones_unlocked: u32,
    pub permanent_bonus: u32,
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
    SpeedBoost,
    LuckBoost,
    PowerBoost,
    SuperBoost,
}

#[contracttype]
pub enum DataKey {
    Config,
    Admin,
    PlayerStreak(Address),
    PlayerCombo(Address),
    PlayerMilestone(Address),
    PlayerBoosts(Address),
    MilestoneThreshold(u32),
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Config {
    pub admin: Address,
    pub paused: bool,
}

// ─────────────────────────────────────────
// CONTRACT
// ─────────────────────────────────────────

#[contract]
pub struct GamificationRewardsContract;

#[contractimpl]
impl GamificationRewardsContract {

    // ✅ FIXED initialize()
    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();

        if env.storage().persistent().has(&DataKey::Config) {
            panic!("Already initialized");
        }

        let config = Config {
            admin: admin.clone(),
            paused: false,
        };

        env.storage().persistent().set(&DataKey::Config, &config);
        env.storage().persistent().set(&DataKey::Admin, &admin);

        // ✅ FIXED milestone calls
        Self::set_milestone_threshold(env.clone(), admin.clone(), 1, 10);
        Self::set_milestone_threshold(env.clone(), admin.clone(), 2, 50);
        Self::set_milestone_threshold(env.clone(), admin.clone(), 3, 100);
        Self::set_milestone_threshold(env.clone(), admin.clone(), 4, 250);
        Self::set_milestone_threshold(env.clone(), admin.clone(), 5, 500);
        Self::set_milestone_threshold(env.clone(), admin.clone(), 6, 1000);
    }

    pub fn set_milestone_threshold(env: Env, admin: Address, level: u32, threshold: u32) {
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::MilestoneThreshold(level), &threshold);
    }

    // ───────── BOOST FIX ─────────

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

            // ✅ FIXED (removed &)
            boosts.set(boost_index, updated_boost);

            env.storage()
                .persistent()
                .set(&DataKey::PlayerBoosts(player.clone()), &boosts);
        }
    }
}