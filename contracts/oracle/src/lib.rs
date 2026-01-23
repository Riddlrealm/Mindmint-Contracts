#![no_std]
use soroban_sdk::{contract, contractimpl, xdr::ToXdr, Address, BytesN, Env, Map, Symbol, Vec};

mod storage;
mod types;

use storage::Storage;
use types::{Config, OracleError, PriceData};

#[contract]
pub struct OracleContract;

#[contractimpl]
impl OracleContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        signers: Vec<BytesN<32>>,
        threshold: u32,
    ) -> Result<(), OracleError> {
        if Storage::has_config(&env) {
            return Err(OracleError::AlreadyInitialized);
        }

        let config = Config {
            admin,
            threshold,
            paused: false,
        };
        Storage::set_config(&env, &config);

        let mut signers_map: Map<BytesN<32>, bool> = Map::new(&env);
        for signer in signers.iter() {
            signers_map.set(signer, true);
        }
        Storage::set_signers(&env, &signers_map);

        Ok(())
    }

    pub fn submit_price(
        env: Env,
        asset: Symbol,
        price: i128,
        timestamp: u64,
        round_id: u64,
        signatures: Vec<(BytesN<32>, BytesN<64>)>, // (Public Key, Signature)
    ) -> Result<(), OracleError> {
        let config = Storage::get_config(&env)?;

        if config.paused {
            return Err(OracleError::Paused);
        }

        if Storage::get_dispute(&env, &asset) {
            return Err(OracleError::Disputed);
        }

        let current_time = env.ledger().timestamp();
        // Allow 60s future drift
        if timestamp > current_time + 60 {
            // allow slight drift
        }

        if let Some(last_data) = Storage::get_price(&env, &asset) {
            if timestamp <= last_data.timestamp {
                return Err(OracleError::StalePrice);
            }
        }

        let signers_map = Storage::get_signers(&env)?;

        let payload_tuple = (
            asset.clone(),
            price,
            timestamp,
            round_id,
            env.current_contract_address(),
        );
        let payload_bytes = payload_tuple.to_xdr(&env);

        let mut valid_signatures = 0;
        let mut used_signers: Map<BytesN<32>, bool> = Map::new(&env);

        for (pub_key, signature) in signatures.iter() {
            if !signers_map.contains_key(pub_key.clone()) {
                continue;
            }
            if used_signers.contains_key(pub_key.clone()) {
                continue;
            }

            env.crypto()
                .ed25519_verify(&pub_key, &payload_bytes, &signature);

            used_signers.set(pub_key, true);
            valid_signatures += 1;
        }

        if valid_signatures < config.threshold {
            return Err(OracleError::InsufficientSignatures);
        }

        let new_data = PriceData {
            price,
            timestamp,
            round_id,
        };
        Storage::set_price(&env, &asset, &new_data);

        Ok(())
    }

    pub fn get_price(env: Env, asset: Symbol) -> Result<PriceData, OracleError> {
        let data = Storage::get_price(&env, &asset).ok_or(OracleError::NotFound)?;

        let current_time = env.ledger().timestamp();
        if current_time > data.timestamp + 3600 {
            return Err(OracleError::StalePrice);
        }

        Ok(data)
    }

    pub fn add_signer(env: Env, signer: BytesN<32>) -> Result<(), OracleError> {
        let config = Storage::get_config(&env)?;
        config.admin.require_auth();

        let mut signers = Storage::get_signers(&env)?;
        signers.set(signer, true);
        Storage::set_signers(&env, &signers);
        Ok(())
    }

    pub fn remove_signer(env: Env, signer: BytesN<32>) -> Result<(), OracleError> {
        let config = Storage::get_config(&env)?;
        config.admin.require_auth();

        let mut signers = Storage::get_signers(&env)?;
        signers.remove(signer);
        Storage::set_signers(&env, &signers);
        Ok(())
    }

    pub fn set_threshold(env: Env, new_threshold: u32) -> Result<(), OracleError> {
        let mut config = Storage::get_config(&env)?;
        config.admin.require_auth();
        config.threshold = new_threshold;
        Storage::set_config(&env, &config);
        Ok(())
    }

    pub fn pause(env: Env) -> Result<(), OracleError> {
        let mut config = Storage::get_config(&env)?;
        config.admin.require_auth();
        config.paused = true;
        Storage::set_config(&env, &config);
        Ok(())
    }

    pub fn unpause(env: Env) -> Result<(), OracleError> {
        let mut config = Storage::get_config(&env)?;
        config.admin.require_auth();
        config.paused = false;
        Storage::set_config(&env, &config);
        Ok(())
    }

    pub fn dispute_feed(env: Env, asset: Symbol) -> Result<(), OracleError> {
        let config = Storage::get_config(&env)?;
        config.admin.require_auth();

        Storage::set_dispute(&env, &asset, true);
        Ok(())
    }

    pub fn resolve_dispute(env: Env, asset: Symbol) -> Result<(), OracleError> {
        let config = Storage::get_config(&env)?;
        config.admin.require_auth();

        Storage::set_dispute(&env, &asset, false);
        Ok(())
    }
}

mod test;
