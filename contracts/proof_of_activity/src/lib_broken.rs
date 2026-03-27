#![no_std]

mod types;
#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, contracterror, vec, Address, Env, Symbol, symbol_short};
use types::{ActivityType};

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

#[contract]
pub struct ProofOfActivityContract;

#[contractimpl]
impl ProofOfActivityContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&symbol_short!("ORACLE_CFG")) {
            return Err(ContractError::AlreadyInitialized);
        }

        env.storage().instance().set(&symbol_short!("ORACLE_CFG"), &admin);
        env.storage().instance().set(&symbol_short!("ORACLES"), &vec![&env, admin.clone()]);
        env.storage().instance().set(&symbol_short!("PROOF_CNT"), &0u64);

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
        
        Self::is_authorized_oracle_internal(&env, &oracle)?;

        let proof_id = Self::get_next_proof_id(&env);
        
        let proof = ActivityProof {
            proof_id,
            player: player.clone(),
            activity_type,
            ref_id: ref_id.clone(),
            timestamp: env.ledger().timestamp(),
            score,
        };

        // Store the proof
        env.storage()
            .persistent()
            .set(&(symbol_short!("PROOF"), proof_id), &proof);

        // Update player's proof count for this activity type
        let count = Self::get_player_proof_count(&env, &player, &activity_type);
        env.storage()
            .persistent()
            .set(&(symbol_short!("COUNT"), player.clone(), activity_type), &(count + 1));

        // Update player's total score
        let current_score = Self::get_activity_score(&env, &player);
        env.storage()
            .persistent()
            .set(&(symbol_short!("SCORE"), player), &(current_score + score));

        // Emit event
        env.events().publish(
            (symbol_short!("ProofRecorded"), proof_id),
            (player, activity_type as u32, score),
        );

        Ok(proof_id)
    }

    pub fn get_proof(env: Env, proof_id: u64) -> Result<ActivityProof, ContractError> {
        env.storage()
            .persistent()
            .get(&(symbol_short!("PROOF"), proof_id))
            .ok_or(ContractError::ProofNotFound)
    }

    pub fn get_player_proofs(
        env: Env,
        player: Address,
        activity_type: ActivityType,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<ActivityProof>, ContractError> {
        if limit > 100 {
            return Err(ContractError::InvalidPagination);
        }

        let total_count = Self::get_player_proof_count(&env, &player, &activity_type);
        let start = offset.min(total_count);
        let end = (offset + limit).min(total_count);

        let mut proofs = vec![&env];
        
        for i in start..end {
            // Note: In a real implementation, you'd need a way to map indices to proof IDs
            // For now, we'll iterate through all proofs and filter
            if let Some(proof) = Self::find_proof_by_index(&env, &player, &activity_type, i) {
                proofs.push_back(proof);
            }
        }

        Ok(proofs)
    }

    pub fn get_activity_score(env: Env, player: Address) -> u64 {
        env.storage()
            .persistent()
            .get(&(symbol_short!("SCORE"), player))
            .unwrap_or(0)
    }

    pub fn add_oracle(env: Env, admin: Address, oracle: Address) -> Result<(), ContractError> {
        admin.require_auth();
        
        Self::is_admin(&env, &admin)?;
        
        let mut config = Self::get_oracle_config(&env)?;
        
        // Check if oracle already exists
        for existing_oracle in config.authorized_oracles.iter() {
            if existing_oracle == oracle {
                return Ok(()); // Already authorized
            }
        }
        
        config.authorized_oracles.push_back(oracle);
        env.storage().instance().set(&DataKey::OracleConfig, &config);

        Ok(())
    }

    pub fn remove_oracle(env: Env, admin: Address, oracle: Address) -> Result<(), ContractError> {
        admin.require_auth();
        
        Self::is_admin(&env, &admin)?;
        
        let mut config = Self::get_oracle_config(&env)?;
        
        // Remove oracle if found
        let mut new_oracles = vec![&env];
        for existing_oracle in config.authorized_oracles.iter() {
            if existing_oracle != oracle {
                new_oracles.push_back(existing_oracle);
            }
        }
        
        config.authorized_oracles = new_oracles;
        env.storage().instance().set(&symbol_short!("ORACLE_CONFIG"), &config);

        Ok(())
    }

    pub fn is_authorized_oracle(env: Env, oracle: Address) -> bool {
        Self::is_authorized_oracle_internal(&env, &oracle).is_ok()
    }
}

impl ProofOfActivityContract {
    fn get_next_proof_id(env: &Env) -> u64 {
        let counter: u64 = env
            .storage()
            .instance()
            .get(&symbol_short!("PROOF_COUNTER"))
            .unwrap_or(0);
        
        let next_id = counter + 1;
        env.storage().instance().set(&symbol_short!("PROOF_COUNTER"), &next_id);
        
        next_id
    }

    fn get_oracle_config(env: &Env) -> Result<OracleConfig, ContractError> {
        env.storage()
            .instance()
            .get(&symbol_short!("ORACLE_CONFIG"))
            .ok_or(ContractError::NotInitialized)
    }

    fn is_admin(env: &Env, address: &Address) -> Result<(), ContractError> {
        let config = Self::get_oracle_config(env)?;
        if config.admin == *address {
            Ok(())
        } else {
            Err(ContractError::Unauthorized)
        }
    }

    fn is_authorized_oracle_internal(env: &Env, oracle: &Address) -> Result<(), ContractError> {
        let config = Self::get_oracle_config(env)?;
        
        for authorized_oracle in config.authorized_oracles.iter() {
            if authorized_oracle == *oracle {
                return Ok(());
            }
        }
        
        Err(ContractError::Unauthorized)
    }

    fn get_player_proof_count(env: &Env, player: &Address, activity_type: &ActivityType) -> u32 {
        env.storage()
            .persistent()
            .get(&(symbol_short!("COUNT"), player.clone(), *activity_type))
            .unwrap_or(0)
    }

    fn find_proof_by_index(
        env: &Env,
        player: &Address,
        activity_type: &ActivityType,
        index: u32,
    ) -> Option<ActivityProof> {
        // This is a simplified implementation
        // In practice, you'd maintain indexed data structures for efficient querying
        let proof_counter: u64 = env
            .storage()
            .instance()
            .get(&symbol_short!("PROOF_COUNTER"))
            .unwrap_or(0);

        let mut found_count = 0;
        for proof_id in 1..=proof_counter {
            if let Some(proof) = env
                .storage()
                .persistent()
                .get::<(Symbol, u64), ActivityProof>(&(symbol_short!("PROOF"), proof_id))
            {
                if proof.player == *player && proof.activity_type == *activity_type {
                    if found_count == index {
                        return Some(proof);
                    }
                    found_count += 1;
                }
            }
        }

        None
    }
}
