#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, symbol_short, Symbol};

#[contracttype]
#[derive(Clone)]
pub struct Achievement {
    pub owner: Address,
    pub puzzle_id: u32,
    pub metadata: String,
    pub timestamp: u64,
}

#[contracttype]
pub enum DataKey {
    Achievement(u32), // Persistent
    NextTokenId,      // Instance
    TotalSupply,      // Instance
    Admin,            // Instance
}

#[contract]
pub struct AchievementNFT;

#[contractimpl]
impl AchievementNFT {
    /// Initialize the contract with an admin
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextTokenId, &1u32);
        env.storage().instance().set(&DataKey::TotalSupply, &0u32);
    }

    /// Mint a new achievement NFT (SEP-41 style)
    pub fn mint(env: Env, to: Address, puzzle_id: u32, metadata: String) -> u32 {
        // In a real scenario, you might check if a 'PuzzleService' contract 
        // confirms this user actually solved the puzzle.
        to.require_auth();

        let token_id: u32 = env.storage().instance().get(&DataKey::NextTokenId).unwrap();

        let achievement = Achievement {
            owner: to.clone(),
            puzzle_id,
            metadata,
            timestamp: env.ledger().timestamp(),
        };

        // Use Persistent storage for the individual NFT data
        env.storage().persistent().set(&DataKey::Achievement(token_id), &achievement);
        
        // Update counters
        env.storage().instance().set(&DataKey::NextTokenId, &(token_id + 1));
        let total: u32 = env.storage().instance().get(&DataKey::TotalSupply).unwrap();
        env.storage().instance().set(&DataKey::TotalSupply, &(total + 1));

        token_id
    }

    /// SEP-41: Transfer ownership
    pub fn transfer(env: Env, from: Address, to: Address, token_id: u32) {
        from.require_auth();

        let mut achievement: Achievement = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(token_id))
            .expect("Token does not exist");

        if achievement.owner != from {
            panic!("Not the owner");
        }

        achievement.owner = to;
        env.storage().persistent().set(&DataKey::Achievement(token_id), &achievement);
    }

    /// SEP-41: Get owner of a token
    pub fn owner_of(env: Env, token_id: u32) -> Address {
        let achievement: Achievement = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(token_id))
            .expect("Token does not exist");
        achievement.owner
    }

    pub fn get_achievement(env: Env, token_id: u32) -> Option<Achievement> {
        env.storage().persistent().get(&DataKey::Achievement(token_id))
    }

    pub fn total_supply(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0)
    }

    /// Burn/Revoke functionality
    pub fn burn(env: Env, token_id: u32) {
        let achievement: Achievement = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(token_id))
            .expect("Token does not exist");
        
        achievement.owner.require_auth();

        env.storage().persistent().remove(&DataKey::Achievement(token_id));
        
        let total: u32 = env.storage().instance().get(&DataKey::TotalSupply).unwrap();
        env.storage().instance().set(&DataKey::TotalSupply, &(total - 1));
    }
}