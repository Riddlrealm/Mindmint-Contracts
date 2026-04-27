#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Map, Symbol, Vec};

mod storage;
mod types;

use storage::Storage;
use types::{Config, PriceFeed, PriceFeedError, PriceSnapshot};

#[contract]
pub struct OraclePriceFeed;

#[contractimpl]
impl OraclePriceFeed {
    pub fn initialize(env: Env, admin: Address, stale_threshold: u64) -> Result<(), PriceFeedError> {
        if Storage::has_config(&env) {
            return Err(PriceFeedError::AlreadyInitialized);
        }

        let config = Config {
            admin,
            stale_threshold,
        };
        Storage::set_config(&env, &config);

        Ok(())
    }

    pub fn register_pair(
        env: Env,
        pair_id: Symbol,
        token_a: Address,
        token_b: Address,
    ) -> Result<(), PriceFeedError> {
        let config = Storage::get_config(&env)?;
        config.admin.require_auth();

        if Storage::has_price_feed(&env, &pair_id) {
            return Err(PriceFeedError::PairNotFound);
        }

        let feed = PriceFeed {
            pair_id: pair_id.clone(),
            token_a,
            token_b,
            providers: Vec::new(&env),
            prices: Map::new(&env),
            last_updated: 0,
            median_price: 0,
        };

        Storage::set_price_feed(&env, &pair_id, &feed);

        Ok(())
    }

    pub fn add_provider(
        env: Env,
        pair_id: Symbol,
        provider: Address,
    ) -> Result<(), PriceFeedError> {
        let config = Storage::get_config(&env)?;
        config.admin.require_auth();

        let mut feed = Storage::get_price_feed(&env, &pair_id)?;

        for existing_provider in feed.providers.iter() {
            if existing_provider == provider {
                return Err(PriceFeedError::ProviderAlreadyExists);
            }
        }

        feed.providers.push_back(provider.clone());
        Storage::set_price_feed(&env, &pair_id, &feed);

        Ok(())
    }

    pub fn remove_provider(
        env: Env,
        pair_id: Symbol,
        provider: Address,
    ) -> Result<(), PriceFeedError> {
        let config = Storage::get_config(&env)?;
        config.admin.require_auth();

        let mut feed = Storage::get_price_feed(&env, &pair_id)?;

        let mut found = false;
        let mut new_providers = Vec::new(&env);
        for existing_provider in feed.providers.iter() {
            if existing_provider == provider {
                found = true;
            } else {
                new_providers.push_back(existing_provider);
            }
        }

        if !found {
            return Err(PriceFeedError::ProviderNotFound);
        }

        feed.providers = new_providers;
        feed.prices.remove(provider.clone());
        Storage::set_price_feed(&env, &pair_id, &feed);

        Ok(())
    }

    pub fn submit_price(
        env: Env,
        pair_id: Symbol,
        provider: Address,
        price: i128,
    ) -> Result<(), PriceFeedError> {
        if price <= 0 {
            return Err(PriceFeedError::InvalidPrice);
        }

        let mut feed = Storage::get_price_feed(&env, &pair_id)?;

        let mut is_authorized = false;
        for existing_provider in feed.providers.iter() {
            if existing_provider == provider {
                is_authorized = true;
                break;
            }
        }

        if !is_authorized {
            return Err(PriceFeedError::Unauthorized);
        }

        feed.prices.set(provider.clone(), price);
        feed.last_updated = env.ledger().timestamp();

        let median = Self::compute_median(&env, &feed)?;
        feed.median_price = median;

        let snapshot = PriceSnapshot {
            median_price: median,
            timestamp: feed.last_updated,
        };
        Storage::add_price_snapshot(&env, &pair_id, &snapshot);

        Storage::set_price_feed(&env, &pair_id, &feed);

        env.events()
            .publish((symbol_short!("price_sub"), pair_id.clone()), (provider, price));
        env.events()
            .publish((symbol_short!("median_up"), pair_id), median);

        Ok(())
    }

    fn compute_median(env: &Env, feed: &PriceFeed) -> Result<i128, PriceFeedError> {
        let count = feed.prices.len();
        if count == 0 {
            return Err(PriceFeedError::InsufficientProviders);
        }

        let mut prices: Vec<i128> = Vec::new(env);
        for (_, price) in feed.prices.iter() {
            prices.push_back(price);
        }

        // Sort prices
        let len = prices.len();
        for i in 0..len {
            for j in i + 1..len {
                if prices.get(i).unwrap() > prices.get(j).unwrap() {
                    let temp = prices.get(i).unwrap();
                    prices.set(i, prices.get(j).unwrap());
                    prices.set(j, temp);
                }
            }
        }

        if len % 2 == 1 {
            // Odd number of providers - return middle
            Ok(prices.get(len / 2).unwrap())
        } else {
            // Even number of providers - return average of two middle values
            let mid1 = prices.get(len / 2 - 1).unwrap();
            let mid2 = prices.get(len / 2).unwrap();
            Ok((mid1 + mid2) / 2)
        }
    }

    pub fn get_price(env: Env, pair_id: Symbol) -> Result<(i128, u64), PriceFeedError> {
        let feed = Storage::get_price_feed(&env, &pair_id)?;
        let config = Storage::get_config(&env)?;

        let current_time = env.ledger().timestamp();
        if current_time > feed.last_updated + config.stale_threshold {
            return Err(PriceFeedError::StalePrice);
        }

        Ok((feed.median_price, feed.last_updated))
    }

    pub fn get_price_history(
        env: Env,
        pair_id: Symbol,
        limit: u32,
    ) -> Result<Vec<PriceSnapshot>, PriceFeedError> {
        Storage::get_price_feed(&env, &pair_id)?;
        Ok(Storage::get_price_history(&env, &pair_id, limit))
    }

    pub fn get_providers(env: Env, pair_id: Symbol) -> Result<Vec<Address>, PriceFeedError> {
        let feed = Storage::get_price_feed(&env, &pair_id)?;
        Ok(feed.providers)
    }

    pub fn set_stale_threshold(env: Env, new_threshold: u64) -> Result<(), PriceFeedError> {
        let mut config = Storage::get_config(&env)?;
        config.admin.require_auth();
        config.stale_threshold = new_threshold;
        Storage::set_config(&env, &config);
        Ok(())
    }
}

mod test;
