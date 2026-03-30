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
    PuzzleCompleted(Address, u32),
}

#[contract]
pub struct AchievementNFT;

#[contractimpl]
impl AchievementNFT {
    //  Initialize
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextTokenId, &1u32);
        env.storage().instance().set(&DataKey::TotalSupply, &0u32);
    }

    //  Mark puzzle completed (admin only)
    pub fn mark_puzzle_completed(env: Env, user: Address, puzzle_id: u32) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let key = DataKey::PuzzleCompleted(user.clone(), puzzle_id);

        env.storage().persistent().set(&key, &true);
        env.storage().persistent().extend_ttl(&key, 100_000, 500_000);
    }

    //  Mint NFT
    pub fn mint(env: Env, to: Address, puzzle_id: u32, metadata: String) -> u32 {
        to.require_auth();

        let completed: bool = env
            .storage()
            .persistent()
            .get(&DataKey::PuzzleCompleted(to.clone(), puzzle_id))
            .unwrap_or(false);

        if !completed {
            panic!("Puzzle not completed");
        }

        Self::mint_internal(env, to, puzzle_id, metadata)
    }

    // 🔹 Craft mint (no auth for testnet)
    pub fn craftmint(env: Env, to: Address, puzzle_id: u32, metadata: String) -> u32 {
        Self::mint_internal(env, to, puzzle_id, metadata)
    }

    // 🔹 Internal mint logic
    fn mint_internal(env: Env, to: Address, puzzle_id: u32, metadata: String) -> u32 {
        let token_id: u32 = env.storage().instance().get(&DataKey::NextTokenId).unwrap();

        let achievement = Achievement {
            owner: to.clone(),
            puzzle_id,
            metadata,
            timestamp: env.ledger().timestamp(),
        };

        // Store NFT
        let key = DataKey::Achievement(token_id);
        env.storage().persistent().set(&key, &achievement);
        env.storage().persistent().extend_ttl(&key, 100_000, 500_000);

        // Update owner collection
        let mut collection = Self::get_collection(env.clone(), to.clone());
        collection.push_back(token_id);

        let collection_key = DataKey::OwnerCollection(to.clone());
        env.storage().persistent().set(&collection_key, &collection);
        env.storage().persistent().extend_ttl(&collection_key, 100_000, 500_000);

        // Update counters
        env.storage().instance().set(&DataKey::NextTokenId, &(token_id + 1));

        let total: u32 = env.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0);
        env.storage().instance().set(&DataKey::TotalSupply, &(total + 1));

        // Emit event
        env.events()
            .publish((symbol_short!("minted"), to.clone()), token_id);

        token_id
    }

    //  Transfer NFT
    pub fn transfer(env: Env, from: Address, to: Address, token_id: u32) {
        from.require_auth();

        if from == to {
            panic!("Cannot transfer to self");
        }

        let mut achievement: Achievement = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(token_id))
            .expect("Token does not exist");

        if achievement.owner != from {
            panic!("Not the owner");
        }

        // Remove from sender
        let mut from_col = Self::get_collection(env.clone(), from.clone());
        let index = from_col.first_index_of(token_id).expect("ID not in collection");
        from_col.remove(index);

        env.storage().persistent().set(&DataKey::OwnerCollection(from.clone()), &from_col);
        env.storage().persistent().extend_ttl(&DataKey::OwnerCollection(from.clone()), 100_000, 500_000);

        // Add to receiver
        let mut to_col = Self::get_collection(env.clone(), to.clone());
        to_col.push_back(token_id);

        env.storage().persistent().set(&DataKey::OwnerCollection(to.clone()), &to_col);
        env.storage().persistent().extend_ttl(&DataKey::OwnerCollection(to.clone()), 100_000, 500_000);

        // Update owner
        achievement.owner = to.clone();
        env.storage().persistent().set(&DataKey::Achievement(token_id), &achievement);
        env.storage().persistent().extend_ttl(&DataKey::Achievement(token_id), 100_000, 500_000);

        env.events().publish((symbol_short!("transfer"), from, to), token_id);
    }

    // 🔹 Get collection
    pub fn get_collection(env: Env, owner: Address) -> Vec<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerCollection(owner))
            .unwrap_or(Vec::new(&env))
    }

    //  Owner of token
    pub fn owner_of(env: Env, token_id: u32) -> Address {
        let achievement: Achievement = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(token_id))
            .expect("Token does not exist");

        achievement.owner
    }

    //Total supply
    pub fn total_supply(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0)
    }

    // Burn NFT
    pub fn burn(env: Env, token_id: u32) {
        let achievement: Achievement = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(token_id))
            .expect("Token does not exist");

        let mut collection = Self::get_collection(env.clone(), achievement.owner.clone());

        if let Some(index) = collection.first_index_of(token_id) {
            collection.remove(index);

            env.storage().persistent().set(
                &DataKey::OwnerCollection(achievement.owner.clone()),
                &collection,
            );
        }

        env.storage().persistent().remove(&DataKey::Achievement(token_id));

        let total: u32 = env.storage().instance().get(&DataKey::TotalSupply).unwrap();
        env.storage().instance().set(&DataKey::TotalSupply, &(total - 1));

        env.events().publish((symbol_short!("burn"), achievement.owner), token_id);
    }

    //  Get full NFT data
    pub fn get_achievement(env: Env, token_id: u32) -> Option<Achievement> {
        env.storage().persistent().get(&DataKey::Achievement(token_id))
    }

    //  Get unique puzzle IDs
    pub fn puzzle_ids_of(env: Env, owner: Address) -> Vec<u32> {
        let token_ids = Self::get_collection(env.clone(), owner);
        let mut puzzles = Vec::new(&env);

        for tid in token_ids.iter() {
            let token_id = tid.clone();

            if let Some(a) = Self::get_achievement(env.clone(), token_id) {
                if !puzzles.contains(&a.puzzle_id) {
                    puzzles.push_back(a.puzzle_id);
                }
            }
        }

        puzzles
    }

    // 🔹 Check puzzle ownership
    pub fn has_puzzle(env: Env, owner: Address, puzzle_id: u32) -> bool {
        let puzzles = Self::puzzle_ids_of(env, owner);
        puzzles.contains(&puzzle_id)
    }
}

mod test;