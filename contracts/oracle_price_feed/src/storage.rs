use crate::types::{Config, PriceFeed, PriceFeedError, PriceSnapshot};
use soroban_sdk::{contracttype, Env, Symbol, Vec};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Global price-feed configuration (instance storage).
    Config,
    /// Per-pair price history, keyed by trading pair id (persistent storage).
    History(Symbol),
}

pub struct Storage;

impl Storage {
    pub fn has_config(env: &Env) -> bool {
        env.storage().instance().has(&DataKey::Config)
    }

    pub fn set_config(env: &Env, config: &Config) {
        env.storage().instance().set(&DataKey::Config, config);
    }

    pub fn get_config(env: &Env) -> Result<Config, PriceFeedError> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(PriceFeedError::NotInitialized)
    }

    pub fn set_price_feed(env: &Env, pair_id: &Symbol, feed: &PriceFeed) {
        env.storage().persistent().set(pair_id, feed);
    }

    pub fn get_price_feed(env: &Env, pair_id: &Symbol) -> Result<PriceFeed, PriceFeedError> {
        env.storage()
            .persistent()
            .get(pair_id)
            .ok_or(PriceFeedError::PairNotFound)
    }

    pub fn has_price_feed(env: &Env, pair_id: &Symbol) -> bool {
        env.storage().persistent().has(pair_id)
    }

    pub fn add_price_snapshot(env: &Env, pair_id: &Symbol, snapshot: &PriceSnapshot) {
        let key = DataKey::History(pair_id.clone());
        let mut history: Vec<PriceSnapshot> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));

        history.push_back(snapshot.clone());

        // Keep only last 100 snapshots
        if history.len() > 100 {
            history.remove(0);
        }

        env.storage().persistent().set(&key, &history);
    }

    pub fn get_price_history(env: &Env, pair_id: &Symbol, limit: u32) -> Vec<PriceSnapshot> {
        let key = DataKey::History(pair_id.clone());
        let history: Vec<PriceSnapshot> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));

        let len = history.len();
        let start = if len > limit as u32 {
            len - limit as u32
        } else {
            0
        };

        let mut result = Vec::new(env);
        for i in start..len {
            result.push_back(history.get(i).unwrap().clone());
        }

        result
    }
}
