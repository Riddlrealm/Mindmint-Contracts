#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Vec,
};

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
    Achievement(u32),      
    OwnerCollection(Address), 
    NextTokenId,           
    TotalSupply,           
    Admin,                 
}

#[contract]
pub struct AchievementNFT;

#[contractimpl]
impl AchievementNFT {
    /// Initialize the contract and set the administrator.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextTokenId, &1u32);
        env.storage().instance().set(&DataKey::TotalSupply, &0u32);
    }

    /// Mints a new achievement NFT.
    pub fn mint(env: Env, to: Address, puzzle_id: u32, metadata: String) -> u32 {
        to.require_auth();

        let token_id: u32 = env.storage().instance().get(&DataKey::NextTokenId).unwrap();

        let achievement = Achievement {
            owner: to.clone(),
            puzzle_id,
            metadata,
            timestamp: env.ledger().timestamp(),
        };

        let key = DataKey::Achievement(token_id);
        env.storage().persistent().set(&key, &achievement);
        env.storage().persistent().extend_ttl(&key, 100_000, 500_000);

        let mut collection = Self::get_collection(env.clone(), to.clone());
        collection.push_back(token_id);
        let collection_key = DataKey::OwnerCollection(to.clone());
        env.storage().persistent().set(&collection_key, &collection);
        env.storage().persistent().extend_ttl(&collection_key, 100_000, 500_000);

        env.storage().instance().set(&DataKey::NextTokenId, &(token_id + 1));
        let total: u32 = env.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0);
        env.storage().instance().set(&DataKey::TotalSupply, &(total + 1));

        env.events().publish((symbol_short!("mint"), to), token_id);

        token_id
    }

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

        let mut from_col = Self::get_collection(env.clone(), from.clone());
        let index = from_col.first_index_of(token_id).expect("ID not in collection");
        from_col.remove(index);
        env.storage().persistent().set(&DataKey::OwnerCollection(from.clone()), &from_col);

        let mut to_col = Self::get_collection(env.clone(), to.clone());
        to_col.push_back(token_id);
        env.storage().persistent().set(&DataKey::OwnerCollection(to.clone()), &to_col);

        achievement.owner = to.clone();
        env.storage().persistent().set(&DataKey::Achievement(token_id), &achievement);

        env.events().publish((symbol_short!("transfer"), from, to), token_id);
    }

    pub fn get_collection(env: Env, owner: Address) -> Vec<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerCollection(owner))
            .unwrap_or(Vec::new(&env))
    }

    pub fn owner_of(env: Env, token_id: u32) -> Address {
        let achievement: Achievement = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(token_id))
            .expect("Token does not exist");
        achievement.owner
    }

    pub fn total_supply(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0)
    }

    pub fn burn(env: Env, token_id: u32) {
        let achievement: Achievement = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(token_id))
            .expect("Token does not exist");
        
        achievement.owner.require_auth();

        let mut collection = Self::get_collection(env.clone(), achievement.owner.clone());
        if let Some(index) = collection.first_index_of(token_id) {
            collection.remove(index);
            env.storage().persistent().set(&DataKey::OwnerCollection(achievement.owner.clone()), &collection);
        }

        env.storage().persistent().remove(&DataKey::Achievement(token_id));
        let total: u32 = env.storage().instance().get(&DataKey::TotalSupply).unwrap();
        env.storage().instance().set(&DataKey::TotalSupply, &(total - 1));

        env.events().publish((symbol_short!("burn"), achievement.owner), token_id);
    }

    pub fn get_achievement(env: Env, token_id: u32) -> Option<Achievement> {
        env.storage().persistent().get(&DataKey::Achievement(token_id))
    }
}

mod test;