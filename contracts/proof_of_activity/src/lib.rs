#![no_std]

mod types;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, vec, Address, Env, Symbol,
    Vec,
};
use types::ActivityType;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    InvalidActivityType = 4,
    ProofNotFound = 5,
    InvalidPagination = 6,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Contract admin / oracle configuration (instance storage).
    Config,
    /// Authorized oracle set (instance storage).
    Oracles,
    /// Count of recorded proofs (instance storage).
    ProofCounter,
    /// Next proof id to assign (instance storage).
    NextProofId,
    /// Per-proof data, keyed by proof id (persistent storage).
    Proof(u64),
    /// Per-player activity count, keyed by (player, activity type) (persistent storage).
    ActivityCount(Address, u32),
    /// Per-player total activity score, keyed by player (persistent storage).
    ActivityScore(Address),
}

#[contract]
pub struct ProofOfActivityContract;

#[contractimpl]
impl ProofOfActivityContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(ContractError::AlreadyInitialized);
        }

        env.storage().instance().set(&DataKey::Config, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Oracles, &vec![&env, admin.clone()]);
        env.storage()
            .instance()
            .set(&DataKey::ProofCounter, &0u64);

        Ok(())
    }

    pub fn record_proof(
        env: Env,
        oracle: Address,
        player: Address,
        activity_type: ActivityType,
        ref_id: Symbol,
        score: u64,
    ) -> Result<u64, ContractError> {
        oracle.require_auth();

        Self::is_authorized(&env, &oracle)?;

        let proof_id = Self::get_next_proof_id(&env);

        // Store proof data as tuple in persistent storage
        let proof_data = (
            player.clone(),
            activity_type as u32,
            ref_id.clone(),
            env.ledger().timestamp(),
            score,
        );
        env.storage()
            .persistent()
            .set(&DataKey::Proof(proof_id), &proof_data);

        // Update player's proof count for this activity type
        let count_key = DataKey::ActivityCount(player.clone(), activity_type as u32);
        let current_count: u32 = env.storage().persistent().get(&count_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&count_key, &(current_count + 1));

        // Update player's total score
        let score_key = DataKey::ActivityScore(player.clone());
        let current_score: u64 = env.storage().persistent().get(&score_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&score_key, &(current_score + score));

        // Emit event
        env.events().publish(
            (symbol_short!("PROOF_REC"), proof_id),
            (player, activity_type as u32, score),
        );

        Ok(proof_id)
    }

    pub fn get_proof(
        env: Env,
        proof_id: u64,
    ) -> Result<(Address, u32, Symbol, u64, u64), ContractError> {
        env.storage()
            .persistent()
            .get(&DataKey::Proof(proof_id))
            .ok_or(ContractError::ProofNotFound)
    }

    pub fn get_player_proofs(
        env: Env,
        player: Address,
        activity_type: ActivityType,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<(Address, u32, Symbol, u64, u64)>, ContractError> {
        if limit > 100 {
            return Err(ContractError::InvalidPagination);
        }

        let mut proofs = vec![&env];
        let proof_counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ProofCounter)
            .unwrap_or(0);

        let mut found_count = 0;
        let mut collected = 0;

        for proof_id in 1..=proof_counter {
            if let Some(proof) = env
                .storage()
                .persistent()
                .get::<DataKey, (Address, u32, Symbol, u64, u64)>(&DataKey::Proof(proof_id))
            {
                if proof.0 == player && proof.1 == activity_type as u32 {
                    if found_count >= offset && collected < limit {
                        proofs.push_back(proof);
                        collected += 1;
                    }
                    found_count += 1;
                }
            }
        }

        Ok(proofs)
    }

    pub fn get_activity_score(env: Env, player: Address) -> u64 {
        let score_key = DataKey::ActivityScore(player);
        env.storage().persistent().get(&score_key).unwrap_or(0)
    }

    pub fn get_activity_count(env: Env, player: Address, activity_type: u32) -> u32 {
        if activity_type > 2 {
            panic!("Invalid activity type");
        }
        let count_key = DataKey::ActivityCount(player, activity_type);
        env.storage().persistent().get(&count_key).unwrap_or(0)
    }

    pub fn add_oracle(env: Env, admin: Address, oracle: Address) -> Result<(), ContractError> {
        admin.require_auth();

        Self::is_admin(&env, &admin)?;

        let mut oracles: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Oracles)
            .unwrap_or_else(|| vec![&env]);

        oracles.push_back(oracle.clone());
        env.storage()
            .instance()
            .set(&DataKey::Oracles, &oracles);

        Ok(())
    }

    pub fn remove_oracle(env: Env, admin: Address, oracle: Address) -> Result<(), ContractError> {
        admin.require_auth();

        Self::is_admin(&env, &admin)?;

        let oracles: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Oracles)
            .unwrap_or_else(|| vec![&env]);

        let mut new_oracles = vec![&env];
        for existing_oracle in oracles.iter() {
            if existing_oracle != oracle {
                new_oracles.push_back(existing_oracle);
            }
        }

        env.storage()
            .instance()
            .set(&DataKey::Oracles, &new_oracles);

        Ok(())
    }

    pub fn is_authorized_oracle(env: Env, oracle: Address) -> bool {
        Self::is_authorized(&env, &oracle).is_ok()
    }

    fn get_next_proof_id(env: &Env) -> u64 {
        let counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ProofCounter)
            .unwrap_or(0);

        let next_id = counter + 1;
        env.storage()
            .instance()
            .set(&DataKey::NextProofId, &next_id);

        next_id
    }

    fn is_admin(env: &Env, address: &Address) -> Result<(), ContractError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(ContractError::NotInitialized)?;

        if admin == *address {
            Ok(())
        } else {
            Err(ContractError::Unauthorized)
        }
    }

    fn is_authorized(env: &Env, oracle: &Address) -> Result<(), ContractError> {
        let oracles: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Oracles)
            .unwrap_or_else(|| vec![env]);

        for authorized_oracle in oracles.iter() {
            if authorized_oracle == *oracle {
                return Ok(());
            }
        }

        Err(ContractError::Unauthorized)
    }
}

#[cfg(test)]
mod datakey_keys_test;
