use soroban_sdk::{contracttype, Env, Symbol};

use crate::types::{AssetSourceConfig, CachedPrice, Config, EmergencyConfig};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Global oracle integration configuration (instance storage).
    Config,
    /// Cached price per asset, keyed by asset symbol (persistent storage).
    Cache(Symbol),
    /// Emergency override configuration (instance storage).
    Emergency,
}

pub struct Storage;

impl Storage {
    pub fn has_config(env: &Env) -> bool {
        env.storage().instance().has(&DataKey::Config)
    }

    pub fn get_config(env: &Env) -> Result<Config, crate::types::IntegrationError> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(crate::types::IntegrationError::NotInitialized)
    }

    pub fn set_config(env: &Env, config: &Config) {
        env.storage().instance().set(&DataKey::Config, config);
    }

    pub fn set_asset_sources(env: &Env, asset: &Symbol, cfg: &AssetSourceConfig) {
        env.storage().persistent().set(asset, cfg);
    }

    pub fn get_asset_sources(
        env: &Env,
        asset: &Symbol,
    ) -> Result<AssetSourceConfig, crate::types::IntegrationError> {
        env.storage()
            .persistent()
            .get(asset)
            .ok_or(crate::types::IntegrationError::SourceNotConfigured)
    }

    pub fn set_cached_price(env: &Env, asset: &Symbol, price: &CachedPrice) {
        env.storage()
            .persistent()
            .set(&DataKey::Cache(asset.clone()), price);
    }

    pub fn get_cached_price(env: &Env, asset: &Symbol) -> Option<CachedPrice> {
        env.storage()
            .persistent()
            .get(&DataKey::Cache(asset.clone()))
    }

    pub fn has_cached_price(env: &Env, asset: &Symbol) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Cache(asset.clone()))
    }

    pub fn get_cached_price_entry(env: &Env, asset: &Symbol) -> Option<CachedPrice> {
        env.storage()
            .persistent()
            .get(&DataKey::Cache(asset.clone()))
    }

    pub fn set_emergency(env: &Env, cfg: &EmergencyConfig) {
        env.storage()
            .instance()
            .set(&DataKey::Emergency, cfg);
    }

    pub fn get_emergency(env: &Env) -> EmergencyConfig {
        env.storage()
            .instance()
            .get(&DataKey::Emergency)
            .unwrap_or(EmergencyConfig {
                active: false,
                price: 0,
                timestamp: 0,
                round_id: 0,
            })
    }
}
