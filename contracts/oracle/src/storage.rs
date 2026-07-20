use crate::types::{Config, OracleError, PriceData};
use soroban_sdk::{contracttype, BytesN, Env, Map, Symbol};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Global oracle configuration (instance storage).
    Config,
    /// Set of authorized signer public keys (instance storage).
    Signers,
    /// Per-asset dispute flag, keyed by asset symbol (persistent storage).
    Dispute(Symbol),
}

pub struct Storage;

impl Storage {
    pub fn has_config(env: &Env) -> bool {
        env.storage().instance().has(&DataKey::Config)
    }

    pub fn set_config(env: &Env, config: &Config) {
        env.storage().instance().set(&DataKey::Config, config);
    }

    pub fn get_config(env: &Env) -> Result<Config, OracleError> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(OracleError::NotInitialized)
    }

    pub fn set_signers(env: &Env, signers: &Map<BytesN<32>, bool>) {
        env.storage()
            .instance()
            .set(&DataKey::Signers, signers);
    }

    pub fn get_signers(env: &Env) -> Result<Map<BytesN<32>, bool>, OracleError> {
        env.storage()
            .instance()
            .get(&DataKey::Signers)
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
            .set(&DataKey::Dispute(asset.clone()), &is_disputed);
    }

    pub fn get_dispute(env: &Env, asset: &Symbol) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Dispute(asset.clone()))
            .unwrap_or(false)
    }
}
