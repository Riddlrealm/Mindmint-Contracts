#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, String, Symbol, Vec,
};

const MAX_ENTRIES: u32 = 100;

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DataKey {
    Config,
    Period(u32),
    PeriodsByContext(String),
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub oracle: Address,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct LeaderboardEntry {
    pub player: Address,
    pub score: u64,
    pub rank: u32,
    pub submitted_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct LeaderboardPeriod {
    pub period_id: u32,
    pub context: String,
    pub entries: Vec<LeaderboardEntry>,
    pub finalized: bool,
    pub finalized_at: Option<u64>,
}

#[contract]
pub struct ImmutableLeaderboard;

#[contractimpl]
impl ImmutableLeaderboard {
    pub fn initialize(env: Env, admin: Address, oracle: Address) {
        admin.require_auth();
        if env.storage().persistent().has(&DataKey::Config) {
            panic!("Already initialized");
        }
        let config = Config { admin, oracle };
        env.storage().persistent().set(&DataKey::Config, &config);
    }

    pub fn submit_score(env: Env, period_id: u32, player: Address, score: u64) {
        let config = Self::get_config(&env);
        config.oracle.require_auth();

        let mut period = Self::must_get_period(&env, period_id);
        
        if period.finalized {
            panic!("Period is finalized");
        }

        let now = env.ledger().timestamp();
        
        // Check if player already has an entry
        let mut existing_index = None;
        for (i, entry) in period.entries.iter().enumerate() {
            if entry.player == player {
                existing_index = Some(i);
                break;
            }
        }

        // Remove existing entry if found
        if let Some(index) = existing_index {
            period.entries.remove(index as u32);
        }

        // Insert new entry at correct position (higher scores first)
        let new_entry = LeaderboardEntry {
            player: player.clone(),
            score,
            rank: 0, // Will be recalculated
            submitted_at: now,
        };

        let mut inserted = false;
        for (i, entry) in period.entries.iter().enumerate() {
            if score > entry.score {
                period.entries.insert(i as u32, new_entry.clone());
                inserted = true;
                break;
            }
        }
        if !inserted {
            // Insert at end if not inserted yet
            period.entries.push_back(new_entry);
        }

        // Enforce top-100 cap
        while period.entries.len() > MAX_ENTRIES {
            period.entries.pop_back();
        }

        // Recalculate ranks
        Self::recalculate_ranks(&mut period, &env);

        // Save updated period
        env.storage()
            .persistent()
            .set(&DataKey::Period(period_id), &period);

        // Find player's rank for event
        let player_rank = period
            .entries
            .iter()
            .position(|entry| entry.player == player)
            .unwrap() as u32 + 1;

        env.events().publish(
            (Symbol::new(&env, "ScoreSubmitted"), period_id),
            (player, score, player_rank),
        );
    }

    pub fn finalize_period(env: Env, admin: Address, period_id: u32) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut period = Self::must_get_period(&env, period_id);
        
        if period.finalized {
            panic!("Already finalized");
        }

        period.finalized = true;
        period.finalized_at = Some(env.ledger().timestamp());

        env.storage()
            .persistent()
            .set(&DataKey::Period(period_id), &period);

        env.events().publish(
            (Symbol::new(&env, "PeriodFinalized"), period_id),
            period.context.clone(),
        );
    }

    pub fn create_period(env: Env, admin: Address, period_id: u32, context: String) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if env.storage().persistent().has(&DataKey::Period(period_id)) {
            panic!("Period already exists");
        }

        let period = LeaderboardPeriod {
            period_id,
            context: context.clone(),
            entries: Vec::new(&env),
            finalized: false,
            finalized_at: None,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Period(period_id), &period);

        // Add to periods by context index
        let mut periods = Self::get_periods_by_context(&env, &context);
        periods.push_back(period_id);
        env.storage()
            .persistent()
            .set(&DataKey::PeriodsByContext(context), &periods);
    }

    pub fn get_leaderboard(env: Env, period_id: u32) -> LeaderboardPeriod {
        Self::must_get_period(&env, period_id)
    }

    pub fn get_player_rank(env: Env, period_id: u32, player: Address) -> Option<(u32, u64)> {
        let period = Self::must_get_period(&env, period_id);
        
        for (i, entry) in period.entries.iter().enumerate() {
            if entry.player == player {
                return Some((i as u32 + 1, entry.score));
            }
        }
        
        None
    }

    pub fn get_all_periods(env: Env, context: String) -> Vec<u32> {
        Self::get_periods_by_context(&env, &context)
    }

    fn recalculate_ranks(period: &mut LeaderboardPeriod, env: &Env) {
        let mut updated_entries = Vec::new(env);
        for (i, entry) in period.entries.iter().enumerate() {
            let mut updated_entry = entry.clone();
            updated_entry.rank = i as u32 + 1;
            updated_entries.push_back(updated_entry);
        }
        period.entries = updated_entries;
    }

    fn must_get_period(env: &Env, period_id: u32) -> LeaderboardPeriod {
        env.storage()
            .persistent()
            .get(&DataKey::Period(period_id))
            .unwrap_or_else(|| panic!("Period not found"))
    }

    fn get_periods_by_context(env: &Env, context: &String) -> Vec<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::PeriodsByContext(context.clone()))
            .unwrap_or_else(|| Vec::new(env))
    }

    fn get_config(env: &Env) -> Config {
        env.storage()
            .persistent()
            .get(&DataKey::Config)
            .unwrap_or_else(|| panic!("Not initialized"))
    }

    fn assert_admin(env: &Env, user: &Address) {
        let config = Self::get_config(env);
        if config.admin != *user {
            panic!("Admin only");
        }
    }
}

#[cfg(test)]
mod test;
