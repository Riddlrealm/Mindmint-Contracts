#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, Map, Symbol, Vec, contract, contractimpl};

#[test]
fn test_nft_composability_flow() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, NftComposabilityContract);
    let client = NftComposabilityContractClient::new(&env, &contract_id);
    
    client.initialize(&admin);
    
    let mut req_traits = Vec::new(&env);
    req_traits.push_back(Symbol::new(&env, "Color"));
    req_traits.push_back(Symbol::new(&env, "Weapon"));
    client.register_traits(&req_traits);
    
    let owner = Address::generate(&env);
    
    let mut traits1 = Map::new(&env);
    traits1.set(Symbol::new(&env, "Color"), Symbol::new(&env, "Red"));
    traits1.set(Symbol::new(&env, "Weapon"), Symbol::new(&env, "Sword"));
    let token1 = client.mint_base(&owner, &traits1);
    
    let mut traits2 = Map::new(&env);
    traits2.set(Symbol::new(&env, "Color"), Symbol::new(&env, "Blue"));
    traits2.set(Symbol::new(&env, "Weapon"), Symbol::new(&env, "Bow"));
    let token2 = client.mint_base(&owner, &traits2);
    
    // Merge: Take Color from 1, Weapon from 2
    let mut selections = Vec::new(&env);
    selections.push_back(TraitSelection { trait_key: Symbol::new(&env, "Color"), source: 1 });
    selections.push_back(TraitSelection { trait_key: Symbol::new(&env, "Weapon"), source: 2 });
    
    let merged_id = client.merge(&owner, &token1, &token2, &selections);
    
    let nft1 = client.get_nft(&token1).unwrap();
    assert!(nft1.is_burned);
    
    let merged_nft = client.get_nft(&merged_id).unwrap();
    assert!(!merged_nft.is_burned);
    assert_eq!(merged_nft.parent_a, token1);
    assert_eq!(merged_nft.parent_b, token2);
    assert_eq!(merged_nft.inherited_traits.get(Symbol::new(&env, "Color")).unwrap(), Symbol::new(&env, "Red"));
    assert_eq!(merged_nft.inherited_traits.get(Symbol::new(&env, "Weapon")).unwrap(), Symbol::new(&env, "Bow"));
    
    // Check lineage
    let lineage = client.get_lineage(&merged_id);
    // Should contain merged_id, token1, token2 (order may vary based on recursion preorder)
    assert!(lineage.contains(&merged_id));
    assert!(lineage.contains(&token1));
    assert!(lineage.contains(&token2));
}

#[test]
#[should_panic(expected = "Selections missing required trait")]
fn test_missing_trait_selection() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, NftComposabilityContract);
    let client = NftComposabilityContractClient::new(&env, &contract_id);
    
    client.initialize(&admin);
    let mut req_traits = Vec::new(&env);
    req_traits.push_back(Symbol::new(&env, "Color"));
    req_traits.push_back(Symbol::new(&env, "Weapon"));
    client.register_traits(&req_traits);
    
    let owner = Address::generate(&env);
    let mut traits = Map::new(&env);
    traits.set(Symbol::new(&env, "Color"), Symbol::new(&env, "Red"));
    traits.set(Symbol::new(&env, "Weapon"), Symbol::new(&env, "Sword"));
    
    let t1 = client.mint_base(&owner, &traits);
    let t2 = client.mint_base(&owner, &traits);
    
    // Only select one trait
    let mut selections = Vec::new(&env);
    selections.push_back(TraitSelection { trait_key: Symbol::new(&env, "Color"), source: 1 });
    
    client.merge(&owner, &t1, &t2, &selections);
}

#[test]
#[should_panic(expected = "Cannot merge burned tokens")]
fn test_double_merge_burned() {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, NftComposabilityContract);
    let client = NftComposabilityContractClient::new(&env, &contract_id);
    
    client.initialize(&admin);
    let mut req_traits = Vec::new(&env);
    req_traits.push_back(Symbol::new(&env, "Color"));
    client.register_traits(&req_traits);
    
    let owner = Address::generate(&env);
    let mut traits = Map::new(&env);
    traits.set(Symbol::new(&env, "Color"), Symbol::new(&env, "Red"));
    
    let t1 = client.mint_base(&owner, &traits);
    let t2 = client.mint_base(&owner, &traits);
    let t3 = client.mint_base(&owner, &traits);
    
    let mut sel = Vec::new(&env);
    sel.push_back(TraitSelection { trait_key: Symbol::new(&env, "Color"), source: 1 });
    
    client.merge(&owner, &t1, &t2, &sel);
    // t1 is now burned, try to merge it again
    client.merge(&owner, &t1, &t3, &sel);
}
