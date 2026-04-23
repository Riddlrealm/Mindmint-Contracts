#![no_std]

mod storage;
#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, Address, Env, Map, Symbol, Vec, symbol_short};
use crate::storage::*;

#[contract]
pub struct NftComposabilityContract;

#[contractimpl]
impl NftComposabilityContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        set_admin(&env, &admin);
    }

    pub fn register_traits(env: Env, traits: Vec<Symbol>) {
        let admin = get_admin(&env);
        admin.require_auth();
        set_required_traits(&env, &traits);
    }

    /// Mint a Gen 0 NFT (for setting up base parents)
    pub fn mint_base(env: Env, owner: Address, traits: Map<Symbol, Symbol>) -> u32 {
        let admin = get_admin(&env);
        admin.require_auth();

        let req_traits = get_required_traits(&env);
        for required_key in req_traits.iter() {
            if !traits.contains_key(required_key) {
                panic!("Missing required trait");
            }
        }

        let id = increment_supply(&env);
        let nft = CompositeNFT {
            token_id: id,
            owner,
            parent_a: 0,
            parent_b: 0,
            inherited_traits: traits,
            merge_timestamp: env.ledger().timestamp(),
            is_burned: false,
        };

        set_token(&env, id, &nft);
        id
    }

    pub fn merge(env: Env, owner: Address, token_a: u32, token_b: u32, selections: Vec<TraitSelection>) -> u32 {
        owner.require_auth();

        if token_a == token_b {
            panic!("Cannot merge identical tokens");
        }

        let parent_a = get_token(&env, token_a).expect("Token A not found");
        let parent_b = get_token(&env, token_b).expect("Token B not found");

        if parent_a.is_burned || parent_b.is_burned {
            panic!("Cannot merge burned tokens");
        }

        if parent_a.owner != owner || parent_b.owner != owner {
            panic!("Not owner of both parents");
        }

        let req_traits = get_required_traits(&env);
        let mut new_traits: Map<Symbol, Symbol> = Map::new(&env);
        
        // Track which traits have been filled
        let mut filled_keys: Map<Symbol, bool> = Map::new(&env);

        for selection in selections.iter() {
            let val = if selection.source == 1 {
                parent_a.inherited_traits.get(selection.trait_key.clone())
            } else if selection.source == 2 {
                parent_b.inherited_traits.get(selection.trait_key.clone())
            } else {
                panic!("Invalid source");
            };

            let trait_val = val.expect("Parent does not have the selected trait");
            new_traits.set(selection.trait_key.clone(), trait_val);
            filled_keys.set(selection.trait_key, true);
        }

        // Validate all required traits are present in the new set
        for required_key in req_traits.iter() {
            if !filled_keys.contains_key(required_key) {
                panic!("Selections missing required trait");
            }
        }

        // Burn parents
        let mut burn_a = parent_a.clone();
        burn_a.is_burned = true;
        set_token(&env, token_a, &burn_a);

        let mut burn_b = parent_b.clone();
        burn_b.is_burned = true;
        set_token(&env, token_b, &burn_b);

        let new_id = increment_supply(&env);
        let new_nft = CompositeNFT {
            token_id: new_id,
            owner: owner.clone(),
            parent_a: token_a,
            parent_b: token_b,
            inherited_traits: new_traits,
            merge_timestamp: env.ledger().timestamp(),
            is_burned: false,
        };

        set_token(&env, new_id, &new_nft);

        env.events().publish((symbol_short!("Merged"), symbol_short!("NFTs")), (token_a, token_b, new_id, owner));

        new_id
    }

    pub fn get_nft(env: Env, token_id: u32) -> Option<CompositeNFT> {
        get_token(&env, token_id)
    }

    pub fn get_lineage(env: Env, token_id: u32) -> Vec<u32> {
        let mut lineage = Vec::new(&env);
        Self::recurse_lineage(&env, token_id, &mut lineage);
        lineage
    }

    fn recurse_lineage(env: &Env, token_id: u32, lineage: &mut Vec<u32>) {
        if token_id == 0 {
            return;
        }
        
        let token_opt = get_token(env, token_id);
        if let Some(token) = token_opt {
            lineage.push_back(token_id);
            if token.parent_a != 0 {
                Self::recurse_lineage(env, token.parent_a, lineage);
            }
            if token.parent_b != 0 {
                Self::recurse_lineage(env, token.parent_b, lineage);
            }
        }
    }
}
