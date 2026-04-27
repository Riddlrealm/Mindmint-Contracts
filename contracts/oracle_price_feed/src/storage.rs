use crate::types::{Config, PriceFeed, PriceFeedError, PriceSnapshot};
use soroban_sdk::{symbol_short, Env, Map, Symbol, Vec};

pub struct Storage;

impl Storage {
    pub fn has_config(env: &Env) -> bool {
        env.storage().instance().has(&symbol_short!("config"))
    }

    pub fn set_config(env: &Env, config: &Config) {
        env.storage()
            .instance()
            .set(&symbol_short!("config"), config);
    }

    pub fn get_config(env: &Env) -> Result<Config, PriceFeedError> {
        env.storage()
            .instance()
            .get(&symbol_short!("config"))
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
        let key = (symbol_short!("history"), pair_id.clone());
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

    pub fn get_price_history(
        env: &Env,
        pair_id: &Symbol,
        limit: u32,
    ) -> Vec<PriceSnapshot> {
        let key = (symbol_short!("history"), pair_id.clone());
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
