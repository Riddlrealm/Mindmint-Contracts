#![cfg(test)]

use soroban_sdk::{Address, Env, String, Map, Symbol};
use soroban_sdk::testutils::{Address as _, Ledger};

use crate::{DynamicNftContract, DynamicNft, EvolutionRule, DataKey};

pub struct DynamicNftContractClient<'a> {
    pub contract_id: soroban_sdk::contractclient::ContractID<'a>,
    pub env: &'a Env,
}

impl<'a> DynamicNftContractClient<'a> {
    pub fn new(env: &'a Env, contract_id: &soroban_sdk::contractclient::ContractID) -> Self {
        Self {
            contract_id: contract_id.clone(),
            env,
        }
    }

    pub fn initialize(&self, admin: &Address, oracle: &Address) {
        self.env.invoke_contract(
            &self.contract_id,
            &Symbol::new(self.env, "initialize"),
            soroban_sdk::vec![self.env, admin.to_val(), oracle.to_val()],
        );
    }

    pub fn mint(&self, owner: &Address) -> u32 {
        self.env.invoke_contract(
            &self.contract_id,
            &Symbol::new(self.env, "mint"),
            soroban_sdk::vec![self.env, owner.to_val()],
        ).try_into_val(self.env).unwrap()
    }

    pub fn add_xp(&self, oracle: &Address, token_id: &u32, amount: &u64) {
        self.env.invoke_contract(
            &self.contract_id,
            &Symbol::new(self.env, "add_xp"),
            soroban_sdk::vec![self.env, oracle.to_val(), token_id.to_val(), amount.to_val()],
        );
    }

    pub fn evolve(&self, caller: &Address, token_id: &u32) {
        self.env.invoke_contract(
            &self.contract_id,
            &Symbol::new(self.env, "evolve"),
            soroban_sdk::vec![self.env, caller.to_val(), token_id.to_val()],
        );
    }

    pub fn get_nft(&self, token_id: &u32) -> Option<DynamicNft> {
        let result = self.env.invoke_contract::<Option<DynamicNft>>(
            &self.contract_id,
            &Symbol::new(self.env, "get_nft"),
            soroban_sdk::vec![self.env, token_id.to_val()],
        );
        result
    }

    pub fn transfer(&self, from: &Address, to: &Address, token_id: &u32) {
        self.env.invoke_contract(
            &self.contract_id,
            &Symbol::new(self.env, "transfer"),
            soroban_sdk::vec![self.env, from.to_val(), to.to_val(), token_id.to_val()],
        );
    }

    pub fn add_evolution_rule(&self, admin: &Address, min_xp: &u64, new_level: &u32, new_metadata_uri: &String) {
        self.env.invoke_contract(
            &self.contract_id,
            &Symbol::new(self.env, "add_evolution_rule"),
            soroban_sdk::vec![self.env, admin.to_val(), min_xp.to_val(), new_level.to_val(), new_metadata_uri.to_val()],
        );
    }

    pub fn remove_evolution_rule(&self, admin: &Address, min_xp: &u64) {
        self.env.invoke_contract(
            &self.contract_id,
            &Symbol::new(self.env, "remove_evolution_rule"),
            soroban_sdk::vec![self.env, admin.to_val(), min_xp.to_val()],
        );
    }

    pub fn get_evolution_rules(&self) -> Map<u64, EvolutionRule> {
        self.env.invoke_contract(
            &self.contract_id,
            &Symbol::new(self.env, "get_evolution_rules"),
            soroban_sdk::vec![self.env],
        ).try_into_val(self.env).unwrap()
    }

    pub fn update_oracle(&self, admin: &Address, new_oracle: &Address) {
        self.env.invoke_contract(
            &self.contract_id,
            &Symbol::new(self.env, "update_oracle"),
            soroban_sdk::vec![self.env, admin.to_val(), new_oracle.to_val()],
        );
    }
}

fn setup() -> (Env, Address, Address, Address, DynamicNftContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, DynamicNftContract);
    let client = DynamicNftContractClient::new(&env, &contract_id);

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    let user = Address::random(&env);

    client.initialize(&admin, &oracle);

    (env, admin, oracle, user, client)
}

#[test]
fn test_initialize() {
    let (env, admin, oracle, _user, client) = setup();
    
    // Verify admin and oracle are set
    let admin_key = DataKey::Admin(admin);
    assert!(env.storage().persistent().has(&admin_key));
    
    let oracle_key = DataKey::Oracle(oracle);
    assert!(env.storage().persistent().has(&oracle_key));
}

#[test]
fn test_mint() {
    let (env, _admin, _oracle, user, client) = setup();
    
    let token_id = client.mint(&user);
    assert_eq!(token_id, 1);
    
    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.token_id, 1);
    assert_eq!(nft.owner, user);
    assert_eq!(nft.level, 1);
    assert_eq!(nft.evolution_stage, 1);
    assert_eq!(nft.xp, 0);
    assert_eq!(nft.metadata_uri, String::from_str(&env, "ipfs://base-metadata"));
}

#[test]
fn test_add_xp() {
    let (env, _admin, oracle, user, client) = setup();
    let token_id = client.mint(&user);
    
    // Add XP as oracle
    client.add_xp(&oracle, &token_id, &100);
    
    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.xp, 100);
}

#[test]
#[should_panic(expected = "Not oracle")]
fn test_add_xp_unauthorized() {
    let (env, _admin, _oracle, user, client) = setup();
    let unauthorized = Address::random(&env);
    let token_id = client.mint(&user);
    
    // Try to add XP as unauthorized user
    client.add_xp(&unauthorized, &token_id, &100);
}

#[test]
fn test_evolution_rule_management() {
    let (env, admin, _oracle, _user, client) = setup();
    
    // Add evolution rule
    let metadata_uri = String::from_str(&env, "ipfs://level2-metadata");
    client.add_evolution_rule(&admin, &100, &2, &metadata_uri);
    
    // Verify rule exists
    let rules = client.get_evolution_rules();
    assert_eq!(rules.len(), 1);
    
    let rule = rules.get(100u64).unwrap();
    assert_eq!(rule.min_xp, 100);
    assert_eq!(rule.new_level, 2);
    assert_eq!(rule.new_metadata_uri, metadata_uri);
    
    // Remove evolution rule
    client.remove_evolution_rule(&admin, &100);
    
    let rules = client.get_evolution_rules();
    assert_eq!(rules.len(), 0);
}

#[test]
fn test_evolution_trigger() {
    let (env, _admin, oracle, user, client) = setup();
    let token_id = client.mint(&user);
    
    // Add evolution rule
    let metadata_uri = String::from_str(&env, "ipfs://level2-metadata");
    client.add_evolution_rule(&admin, &100, &2, &metadata_uri);
    
    // Add XP to trigger evolution
    client.add_xp(&oracle, &token_id, &150);
    
    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.level, 2);
    assert_eq!(nft.evolution_stage, 2);
    assert_eq!(nft.metadata_uri, metadata_uri);
    assert_eq!(nft.xp, 150);
}

#[test]
fn test_manual_evolution() {
    let (env, admin, oracle, user, client) = setup();
    let token_id = client.mint(&user);
    
    // Add evolution rule
    let metadata_uri = String::from_str(&env, "ipfs://level2-metadata");
    client.add_evolution_rule(&admin, &100, &2, &metadata_uri);
    
    // Add XP below threshold
    client.add_xp(&oracle, &token_id, &50);
    
    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.level, 1); // Should not have evolved
    
    // Add more XP to reach threshold
    client.add_xp(&oracle, &token_id, &50);
    
    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.level, 2); // Should have evolved
}

#[test]
fn test_soulbound_restriction() {
    let (env, admin, oracle, user, client) = setup();
    let recipient = Address::generate(&env);
    let token_id = client.mint(&user);
    
    // Try to transfer at level 1 (should fail)
    let result = std::panic::catch_unwind(|| {
        client.transfer(&user, &recipient, &token_id);
    });
    assert!(result.is_err());
    
    // Level up to 3
    let metadata_uri2 = String::from_str(&env, "ipfs://level2-metadata");
    let metadata_uri3 = String::from_str(&env, "ipfs://level3-metadata");
    client.add_evolution_rule(&admin, &100, &2, &metadata_uri2);
    client.add_evolution_rule(&admin, &200, &3, &metadata_uri3);
    
    client.add_xp(&oracle, &token_id, &200);
    
    // Now transfer should succeed
    client.transfer(&user, &recipient, &token_id);
    
    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.owner, recipient);
}

#[test]
fn test_max_level_cap() {
    let (env, admin, oracle, user, client) = setup();
    let token_id = client.mint(&user);
    
    // Add evolution rules up to level 5
    client.add_evolution_rule(&admin, &100, &2, &String::from_str(&env, "ipfs://level2"));
    client.add_evolution_rule(&admin, &200, &3, &String::from_str(&env, "ipfs://level3"));
    client.add_evolution_rule(&admin, &300, &4, &String::from_str(&env, "ipfs://level4"));
    client.add_evolution_rule(&admin, &400, &5, &String::from_str(&env, "ipfs://level5"));
    
    // Add XP to reach max level
    client.add_xp(&oracle, &token_id, &500);
    
    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.level, 5);
    
    // Add more XP - should not evolve further (no more rules)
    client.add_xp(&oracle, &token_id, &100);
    
    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.level, 5); // Should stay at max level
}

#[test]
fn test_oracle_update() {
    let (env, admin, oracle, user, client) = setup();
    let new_oracle = Address::generate(&env);
    let token_id = client.mint(&user);
    
    // Old oracle should be able to add XP
    client.add_xp(&oracle, &token_id, &50);
    
    // Update oracle
    client.update_oracle(&admin, &new_oracle);
    
    // Old oracle should no longer be able to add XP
    let result = std::panic::catch_unwind(|| {
        client.add_xp(&oracle, &token_id, &50);
    });
    assert!(result.is_err());
    
    // New oracle should be able to add XP
    client.add_xp(&new_oracle, &token_id, &50);
    
    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.xp, 100);
}

#[test]
fn test_event_emission() {
    let (env, admin, oracle, user, client) = setup();
    
    // Test mint event
    let token_id = client.mint(&user);
    
    // Test XP added event
    client.add_xp(&oracle, &token_id, &100);
    
    // Add evolution rule
    let metadata_uri = String::from_str(&env, "ipfs://level2-metadata");
    client.add_evolution_rule(&admin, &50, &2, &metadata_uri);
    
    // Test evolution event
    client.add_xp(&oracle, &token_id, &50);
    
    // Verify events were published (in a real test, you'd check the event logs)
    // For now, just ensure the contract executes without panicking
    let nft = client.get_nft(&token_id).unwrap();
    assert_eq!(nft.level, 2);
}

#[test]
#[should_panic(expected = "Only owner can trigger evolution")]
fn test_evolve_unauthorized() {
    let (env, _admin, _oracle, user, client) = setup();
    let unauthorized = Address::random(&env);
    let token_id = client.mint(&user);
    
    // Try to evolve as unauthorized user
    client.evolve(&unauthorized, &token_id);
}

#[test]
#[should_panic(expected = "Not admin")]
fn test_evolution_rule_unauthorized() {
    let (env, _admin, _oracle, _user, client) = setup();
    let unauthorized = Address::random(&env);
    
    // Try to add evolution rule as unauthorized user
    client.add_evolution_rule(&unauthorized, &100, &2, &String::from_str(&env, "ipfs://level2"));
}
