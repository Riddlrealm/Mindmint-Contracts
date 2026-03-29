#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Vec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum RewardType {
    Token = 0,
    Cosmetic = 1,
    Nft = 2,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct PassTier {
    pub required_xp: u32,
    pub reward_type: RewardType,
    pub reward_amount: u128,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct BattlePass {
    pub season_id: u32,
    pub holder: Address,
    pub xp: u32,
    pub tier_reached: u32,
    pub rewards_claimed: Vec<u32>,
    pub purchase_time: u64,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct Season {
    pub season_id: u32,
    pub start_at: u64,
    pub end_at: u64,
    pub price: u128,
    pub tiers: Vec<PassTier>,
    pub is_active: bool,
    pub oracle_address: Address,
}

#[derive(Clone, Debug)]
#[contracttype]
pub enum DataKey {
    Season(u32),                    // season_id -> Season
    BattlePass(u32),                // pass_id -> BattlePass  
    PlayerPass(Address, u32),      // (player, season_id) -> pass_id
    NextPassId,                    // u32 - auto-increment
    Admin,                         // Address
}

#[contract]
pub struct BattlePassContract;

#[contractimpl]
impl BattlePassContract {
    /// Initialize contract with admin address
    pub fn init(env: Env, admin: Address) {
        if env.storage().persistent().has(&DataKey::Admin) {
            panic!("Contract already initialized");
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::NextPassId, &1u32);
    }

    /// Create a new season (admin only)
    pub fn create_season(
        env: Env,
        season_id: u32,
        start_at: u64,
        end_at: u64,
        price: u128,
        oracle_address: Address,
    ) {
        let admin = Self::get_admin(&env);
        admin.require_auth();
        
        // Check if season already exists
        if env.storage().persistent().has(&DataKey::Season(season_id)) {
            panic!("Season already exists");
        }
        
        // Validate time window
        if start_at >= end_at {
            panic!("Invalid time window");
        }
        
        let season = Season {
            season_id,
            start_at,
            end_at,
            price,
            tiers: Vec::new(&env),
            is_active: false,
            oracle_address,
        };
        
        env.storage().persistent().set(&DataKey::Season(season_id), &season);
    }
    
    /// Configure tiers for a season (admin only, before season starts)
    pub fn configure_season_tiers(env: Env, season_id: u32, tiers: Vec<PassTier>) {
        let admin = Self::get_admin(&env);
        admin.require_auth();
        
        let mut season = Self::get_season(&env, season_id);
        
        // Can only configure before season starts
        if env.ledger().timestamp() >= season.start_at {
            panic!("Cannot configure after season start");
        }
        
        // Validate tiers are in ascending order by required_xp
        for i in 1..tiers.len() {
            if tiers.get(i).unwrap().required_xp <= tiers.get(i - 1).unwrap().required_xp {
                panic!("Tiers must be in ascending order by required_xp");
            }
        }
        
        season.tiers = tiers;
        env.storage().persistent().set(&DataKey::Season(season_id), &season);
    }
    
    /// Activate a season (admin only)
    pub fn activate_season(env: Env, season_id: u32) {
        let admin = Self::get_admin(&env);
        admin.require_auth();
        
        let mut season = Self::get_season(&env, season_id);
        season.is_active = true;
        env.storage().persistent().set(&DataKey::Season(season_id), &season);
    }

    /// Purchase a battle pass for a specific season
    pub fn purchase_pass(env: Env, buyer: Address, season_id: u32) {
        buyer.require_auth();
        
        let season = Self::get_season(&env, season_id);
        
        // Check season window
        let now = env.ledger().timestamp();
        if now < season.start_at || now > season.end_at {
            panic!("Season is not active");
        }
        
        if !season.is_active {
            panic!("Season is not active");
        }
        
        // Check if already owns a pass this season
        if env.storage().persistent().has(&DataKey::PlayerPass(buyer.clone(), season_id)) {
            panic!("Already owns a pass for this season");
        }
        
        // TODO: Implement price deduction - this would require token integration
        // For now, we'll just create the pass record
        
        // Create new battle pass
        let pass_id = Self::get_next_pass_id(&env);
        let battle_pass = BattlePass {
            season_id,
            holder: buyer.clone(),
            xp: 0,
            tier_reached: 0,
            rewards_claimed: Vec::new(&env),
            purchase_time: now,
        };
        
        // Store pass and mappings
        env.storage().persistent().set(&DataKey::BattlePass(pass_id), &battle_pass);
        env.storage().persistent().set(&DataKey::PlayerPass(buyer, season_id), &pass_id);
        
        // Increment pass ID counter
        env.storage().persistent().set(&DataKey::NextPassId, &(pass_id + 1));
    }

    /// Add XP to a specific battle pass (oracle authorized)
    pub fn add_xp(env: Env, pass_id: u32, amount: u32) {
        let battle_pass = Self::get_battle_pass(&env, pass_id);
        let season = Self::get_season(&env, battle_pass.season_id);
        
        // Check season window
        let now = env.ledger().timestamp();
        if now < season.start_at || now > season.end_at {
            panic!("Season is not active");
        }
        
        // Oracle authorization required
        season.oracle_address.require_auth();
        
        // Update XP and calculate new tier
        let new_xp = battle_pass.xp.saturating_add(amount);
        let new_tier = Self::calculate_tier_from_xp(&season, new_xp);
        
        let mut updated_pass = battle_pass;
        updated_pass.xp = new_xp;
        
        // Check if tier advanced and emit event
        if new_tier > updated_pass.tier_reached {
            updated_pass.tier_reached = new_tier;
            
            // Emit TierReached event
            env.events().publish(
                ("TierReached", pass_id),
                (updated_pass.tier_reached, new_xp),
            );
        }
        
        env.storage().persistent().set(&DataKey::BattlePass(pass_id), &updated_pass);
    }

    /// Claim tier reward for a specific battle pass
    pub fn claim_tier_reward(env: Env, pass_id: u32, tier_index: u32) -> PassTier {
        let battle_pass = Self::get_battle_pass(&env, pass_id);
        let season = Self::get_season(&env, battle_pass.season_id);
        
        // Check season window
        let now = env.ledger().timestamp();
        if now < season.start_at || now > season.end_at {
            panic!("Season is not active");
        }
        
        // Check if tier is reached
        if tier_index > battle_pass.tier_reached {
            panic!("Tier not reached yet");
        }
        
        // Check if tier exists
        if tier_index >= season.tiers.len() as u32 {
            panic!("Invalid tier index");
        }
        
        // Check if already claimed
        if battle_pass.rewards_claimed.contains(&tier_index) {
            panic!("Reward already claimed");
        }
        
        let tier_reward = season.tiers.get(tier_index as u32).unwrap().clone();
        
        // Mark as claimed
        let mut updated_pass = battle_pass;
        updated_pass.rewards_claimed.push_back(tier_index);
        env.storage().persistent().set(&DataKey::BattlePass(pass_id), &updated_pass);
        
        // Emit RewardClaimed event
        env.events().publish(
            ("RewardClaimed", pass_id),
            (tier_index, tier_reward.reward_amount, tier_reward.reward_type as u32),
        );
        
        tier_reward
    }

    /// Get battle pass information
    pub fn get_pass(env: Env, pass_id: u32) -> (u32, u32, Vec<u32>) {
        let battle_pass = Self::get_battle_pass(&env, pass_id);
        (battle_pass.xp, battle_pass.tier_reached, battle_pass.rewards_claimed)
    }
    
    /// Helper functions
    fn get_admin(env: &Env) -> Address {
        env.storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Admin not set"))
    }
    
    fn get_season(env: &Env, season_id: u32) -> Season {
        env.storage()
            .persistent()
            .get(&DataKey::Season(season_id))
            .unwrap_or_else(|| panic!("Season not found"))
    }
    
    fn get_battle_pass(env: &Env, pass_id: u32) -> BattlePass {
        env.storage()
            .persistent()
            .get(&DataKey::BattlePass(pass_id))
            .unwrap_or_else(|| panic!("Battle pass not found"))
    }
    
    fn get_next_pass_id(env: &Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::NextPassId)
            .unwrap_or(1)
    }
    
    fn calculate_tier_from_xp(season: &Season, xp: u32) -> u32 {
        let mut tier = 0;
        for (i, pass_tier) in season.tiers.iter().enumerate() {
            if xp >= pass_tier.required_xp {
                tier = i as u32;
            } else {
                break;
            }
        }
        tier
    }
}
