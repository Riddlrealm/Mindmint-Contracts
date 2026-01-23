use crate::types::{Config, OracleError, PriceData};
use soroban_sdk::{symbol_short, BytesN, Env, Map, Symbol};

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

    pub fn get_config(env: &Env) -> Result<Config, OracleError> {
        env.storage()
            .instance()
            .get(&symbol_short!("config"))
            .ok_or(OracleError::NotInitialized)
    }

    pub fn set_signers(env: &Env, signers: &Map<BytesN<32>, bool>) {
        env.storage()
            .instance()
            .set(&symbol_short!("signers"), signers);
    }

    pub fn get_signers(env: &Env) -> Result<Map<BytesN<32>, bool>, OracleError> {
        env.storage()
            .instance()
            .get(&symbol_short!("signers"))
            .ok_or(OracleError::NotInitialized) // Should be initialized if config is
    }

    pub fn set_price(env: &Env, asset: &Symbol, data: &PriceData) {
        env.storage().persistent().set(asset, data);
    }

    pub fn get_price(env: &Env, asset: &Symbol) -> Option<PriceData> {
        env.storage().persistent().get(asset)
    }

    pub fn set_dispute(env: &Env, asset: &Symbol, is_disputed: bool) {
        env.storage()
            .persistent()
            .set(&(symbol_short!("dispute"), asset.clone()), &is_disputed);
    }

    pub fn get_dispute(env: &Env, asset: &Symbol) -> bool {
        env.storage()
            .persistent()
            .get(&(symbol_short!("dispute"), asset.clone()))
            .unwrap_or(false)
    }
}
