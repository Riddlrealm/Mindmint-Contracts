#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, contractevent, Address, Env, String, Map};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicNft {
    pub token_id: u32,
    pub owner: Address,
    pub level: u32,
    pub evolution_stage: u32,
    pub metadata_uri: String,
    pub xp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvolutionRule {
    pub min_xp: u64,
    pub new_level: u32,
    pub new_metadata_uri: String,
}

#[contracttype]
pub enum DataKey {
    Admin(Address),
    Oracle(Address),
    DynamicNft(u32),
    NextTokenId,
    EvolutionRules,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct NFTEvolved {
    pub token_id: u32,
    pub old_level: u32,
    pub new_level: u32,
    pub new_metadata_uri: String,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct NFTMinted {
    pub token_id: u32,
    pub owner: Address,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct XPAdded {
    pub token_id: u32,
    pub amount: u64,
    pub total_xp: u64,
}

#[contract]
pub struct DynamicNftContract;

#[contractimpl]
impl DynamicNftContract {
    pub fn initialize(env: Env, admin: Address, oracle: Address) {
        admin.require_auth();
        
        if env.storage().persistent().has(&DataKey::Admin(admin.clone())) {
            panic!("Already initialized");
        }
        
        env.storage().persistent().set(&DataKey::Admin(admin.clone()), &true);
        env.storage().persistent().set(&DataKey::Oracle(oracle), &true);
        env.storage().persistent().set(&DataKey::NextTokenId, &1u32);
        env.storage().persistent().set(&DataKey::EvolutionRules, &Map::<u64, EvolutionRule>::new(&env));
    }

    pub fn mint(env: Env, owner: Address) -> u32 {
        owner.require_auth();
        
        let next_id: u32 = env.storage().persistent().get(&DataKey::NextTokenId).unwrap();
        
        let nft = DynamicNft {
            token_id: next_id,
            owner: owner.clone(),
            level: 1,
            evolution_stage: 1,
            metadata_uri: String::from_str(&env, "ipfs://base-metadata"),
            xp: 0,
        };
        
        env.storage().persistent().set(&DataKey::DynamicNft(next_id), &nft);
        env.storage().persistent().set(&DataKey::NextTokenId, &(next_id + 1));
        
        env.events().publish_event(&NFTMinted { token_id: next_id, owner });
        
        next_id
    }

    pub fn add_xp(env: Env, oracle: Address, token_id: u32, amount: u64) {
        oracle.require_auth();
        Self::assert_oracle(&env, &oracle);
        
        let mut nft: DynamicNft = env.storage().persistent()
            .get(&DataKey::DynamicNft(token_id))
            .unwrap_or_else(|| panic!("NFT not found"));
        
        nft.xp = nft.xp.saturating_add(amount);
        let total_xp = nft.xp;
        
        env.storage().persistent().set(&DataKey::DynamicNft(token_id), &nft);
        
        env.events().publish_event(&XPAdded { token_id, amount, total_xp });
        
        // Check for evolution
        Self::check_and_evolve(&env, token_id);
    }

    pub fn evolve(env: Env, caller: Address, token_id: u32) {
        caller.require_auth();
        
        let nft: DynamicNft = env.storage().persistent()
            .get(&DataKey::DynamicNft(token_id))
            .unwrap_or_else(|| panic!("NFT not found"));
        
        if nft.owner != caller {
            panic!("Only owner can trigger evolution");
        }
        
        Self::check_and_evolve(&env, token_id);
    }

    pub fn get_nft(env: Env, token_id: u32) -> Option<DynamicNft> {
        env.storage().persistent().get(&DataKey::DynamicNft(token_id))
    }

    pub fn transfer(env: Env, from: Address, to: Address, token_id: u32) {
        from.require_auth();
        
        let mut nft: DynamicNft = env.storage().persistent()
            .get(&DataKey::DynamicNft(token_id))
            .unwrap_or_else(|| panic!("NFT not found"));
        
        if nft.owner != from {
            panic!("Not the owner");
        }
        
        // Soulbound restriction: non-transferable until level 3
        if nft.level < 3 {
            panic!("NFT is soulbound until level 3");
        }
        
        nft.owner = to.clone();
        env.storage().persistent().set(&DataKey::DynamicNft(token_id), &nft);
    }

    pub fn add_evolution_rule(env: Env, admin: Address, min_xp: u64, new_level: u32, new_metadata_uri: String) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        
        let mut rules: Map<u64, EvolutionRule> = env.storage().persistent()
            .get(&DataKey::EvolutionRules)
            .unwrap_or_else(|| Map::new(&env));
        
        let rule = EvolutionRule {
            min_xp,
            new_level,
            new_metadata_uri: new_metadata_uri.clone(),
        };
        
        rules.set(min_xp, rule);
        env.storage().persistent().set(&DataKey::EvolutionRules, &rules);
    }

    pub fn remove_evolution_rule(env: Env, admin: Address, min_xp: u64) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        
        let mut rules: Map<u64, EvolutionRule> = env.storage().persistent()
            .get(&DataKey::EvolutionRules)
            .unwrap_or_else(|| Map::new(&env));
        
        rules.remove(min_xp);
        env.storage().persistent().set(&DataKey::EvolutionRules, &rules);
    }

    pub fn get_evolution_rules(env: Env) -> Map<u64, EvolutionRule> {
        env.storage().persistent()
            .get(&DataKey::EvolutionRules)
            .unwrap_or_else(|| Map::new(&env))
    }

    pub fn update_oracle(env: Env, admin: Address, new_oracle: Address) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        
        // Remove old oracle
        let old_oracle: Option<Address> = env.storage().persistent()
            .get(&DataKey::Oracle(new_oracle.clone()));
        
        if let Some(old) = old_oracle {
            env.storage().persistent().remove(&DataKey::Oracle(old));
        }
        
        // Set new oracle
        env.storage().persistent().set(&DataKey::Oracle(new_oracle.clone()), &true);
    }

    fn check_and_evolve(env: &Env, token_id: u32) {
        let mut nft: DynamicNft = env.storage().persistent()
            .get(&DataKey::DynamicNft(token_id))
            .unwrap_or_else(|| panic!("NFT not found"));
        
        let rules: Map<u64, EvolutionRule> = env.storage().persistent()
            .get(&DataKey::EvolutionRules)
            .unwrap_or_else(|| Map::new(env));
        
        let old_level = nft.level;
        
        // Find the highest evolution rule that can be applied
        let mut applicable_rule: Option<EvolutionRule> = None;
        let mut highest_min_xp = 0u64;
        
        for (min_xp, rule) in rules.iter() {
            if nft.xp >= min_xp && min_xp > highest_min_xp && rule.new_level > nft.level {
                highest_min_xp = min_xp;
                applicable_rule = Some(rule);
            }
        }
        
        if let Some(rule) = applicable_rule {
            nft.level = rule.new_level;
            nft.evolution_stage += 1;
            nft.metadata_uri = rule.new_metadata_uri.clone();
            
            env.storage().persistent().set(&DataKey::DynamicNft(token_id), &nft);
            
            env.events().publish_event(&NFTEvolved {
                token_id,
                old_level,
                new_level: rule.new_level,
                new_metadata_uri: rule.new_metadata_uri,
            });
        }
    }

    fn assert_admin(env: &Env, admin: &Address) {
        let is_admin: bool = env.storage().persistent()
            .get(&DataKey::Admin(admin.clone()))
            .unwrap_or(false);
        if !is_admin {
            panic!("Not admin");
        }
    }

    fn assert_oracle(env: &Env, oracle: &Address) {
        let is_oracle: bool = env.storage().persistent()
            .get(&DataKey::Oracle(oracle.clone()))
            .unwrap_or(false);
        if !is_oracle {
            panic!("Not oracle");
        }
    }
}
