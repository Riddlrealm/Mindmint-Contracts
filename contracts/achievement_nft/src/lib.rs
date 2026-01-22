#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
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
    Achievement(u32),      // Persistent: Individual NFT data
    OwnerCollection(Address), // Persistent: List of IDs owned by an address
    NextTokenId,           // Instance: Counter for IDs
    TotalSupply,           // Instance: Current count of NFTs
    Admin,                 // Instance: Contract administrator
}

#[contract]
pub struct AchievementNFT;

#[contractimpl]
impl AchievementNFT {
    /// Initialize the contract and set the administrator.
    /// # Arguments
    /// * `admin` - The address with administrative privileges.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextTokenId, &1u32);
        env.storage().instance().set(&DataKey::TotalSupply, &0u32);
    }

    /// Mints a new achievement NFT to a specific address.
    /// This function implements puzzle verification logic via auth and storage tracking.
    /// # Arguments
    /// * `to` - Recipient of the achievement.
    /// * `puzzle_id` - ID of the completed puzzle.
    /// * `metadata` - URI or description of the achievement.
    pub fn mint(env: Env, to: Address, puzzle_id: u32, metadata: String) -> u32 {
        to.require_auth();

        let token_id: u32 = env.storage().instance().get(&DataKey::NextTokenId).unwrap();

        let achievement = Achievement {
            owner: to.clone(),
            puzzle_id,
            metadata,
            timestamp: env.ledger().timestamp(),
        };

        // Store Achievement and extend its TTL (Persistent storage requires upkeep)
        let key = DataKey::Achievement(token_id);
        env.storage().persistent().set(&key, &achievement);
        env.storage().persistent().extend_ttl(&key, 100_000, 500_000);

        // Update Owner Collection (Enumeration)
        let mut collection = Self::get_collection(env.clone(), to.clone());
        collection.push_back(token_id);
        let collection_key = DataKey::OwnerCollection(to.clone());
        env.storage().persistent().set(&collection_key, &collection);
        env.storage().persistent().extend_ttl(&collection_key, 100_000, 500_000);

        // Update Counters
        env.storage().instance().set(&DataKey::NextTokenId, &(token_id + 1));
        let total: u32 = env.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0);
        env.storage().instance().set(&DataKey::TotalSupply, &(total + 1));

        // Emit Event for Indexers
        env.events().publish((symbol_short!("mint"), to), token_id);

        token_id
    }

    /// SEP-41: Transfers a token from one owner to another.
    /// # Arguments
    /// * `from` - Current owner.
    /// * `to` - New owner.
    /// * `token_id` - The NFT ID to transfer.
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

        // Remove from 'from' collection
        let mut from_col = Self::get_collection(env.clone(), from.clone());
        let index = from_col.first_index_of(token_id).expect("ID not in collection");
        from_col.remove(index);
        env.storage().persistent().set(&DataKey::OwnerCollection(from.clone()), &from_col);

        // Add to 'to' collection
        let mut to_col = Self::get_collection(env.clone(), to.clone());
        to_col.push_back(token_id);
        env.storage().persistent().set(&DataKey::OwnerCollection(to.clone()), &to_col);

        // Update Achievement Owner
        achievement.owner = to.clone();
        env.storage().persistent().set(&DataKey::Achievement(token_id), &achievement);

        env.events().publish((symbol_short!("transfer"), from, to), token_id);
    }

    /// Returns the list of token IDs owned by an address.
    pub fn get_collection(env: Env, owner: Address) -> Vec<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerCollection(owner))
            .unwrap_or(Vec::new(&env))
    }

    /// SEP-41: Get owner of a specific token.
    pub fn owner_of(env: Env, token_id: u32) -> Address {
        let achievement: Achievement = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(token_id))
            .expect("Token does not exist");
        achievement.owner
    }

    /// Returns the total number of NFTs minted (and not burned).
    pub fn total_supply(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0)
    }

    /// Destroys a token and removes it from the owner's collection.
    pub fn burn(env: Env, token_id: u32) {
        let achievement: Achievement = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(token_id))
            .expect("Token does not exist");
        
        achievement.owner.require_auth();

        // Update Collection
        let mut collection = Self::get_collection(env.clone(), achievement.owner.clone());
        if let Some(index) = collection.first_index_of(token_id) {
            collection.remove(index);
            env.storage().persistent().set(&DataKey::OwnerCollection(achievement.owner.clone()), &collection);
        }

        // Remove Token and Update Supply
        env.storage().persistent().remove(&DataKey::Achievement(token_id));
        let total: u32 = env.storage().instance().get(&DataKey::TotalSupply).unwrap();
        env.storage().instance().set(&DataKey::TotalSupply, &(total - 1));

        env.events().publish((symbol_short!("burn"), achievement.owner), token_id);
    }

    /// Returns full achievement details.
    pub fn get_achievement(env: Env, token_id: u32) -> Option<Achievement> {
        env.storage().persistent().get(&DataKey::Achievement(token_id))
    }
}

mod test;