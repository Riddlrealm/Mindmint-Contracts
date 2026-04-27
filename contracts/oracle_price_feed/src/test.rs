#![cfg(test)]
extern crate std;
use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger, Address, Symbol};

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, OraclePriceFeed);
    let client = OraclePriceFeedClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin, &300);

    // Test duplicate initialization
    let result = client.try_initialize(&admin, &300);
    assert!(result.is_err());
}

#[test]
fn test_register_pair() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, OraclePriceFeed);
    let client = OraclePriceFeedClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin, &300);

    let token_a = Address::generate(&env);
    let token_b = Address::generate(&env);
    let pair_id = Symbol::new(&env, "XLM_USDC");

    client.register_pair(&pair_id, &token_a, &token_b);

    // Test duplicate registration
    let result = client.try_register_pair(&pair_id, &token_a, &token_b);
    assert!(result.is_err());
}

#[test]
fn test_provider_management() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, OraclePriceFeed);
    let client = OraclePriceFeedClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin, &300);

    let token_a = Address::generate(&env);
    let token_b = Address::generate(&env);
    let pair_id = Symbol::new(&env, "XLM_USDC");

    client.register_pair(&pair_id, &token_a, &token_b);

    let provider1 = Address::generate(&env);
    let provider2 = Address::generate(&env);

    client.add_provider(&pair_id, &provider1);
    client.add_provider(&pair_id, &provider2);

    let providers = client.get_providers(&pair_id);
    assert_eq!(providers.len(), 2);

    // Test duplicate provider
    let result = client.try_add_provider(&pair_id, &provider1);
    assert!(result.is_err());

    // Test remove provider
    client.remove_provider(&pair_id, &provider1);
    let providers = client.get_providers(&pair_id);
    assert_eq!(providers.len(), 1);

    // Test remove non-existent provider
    let result = client.try_remove_provider(&pair_id, &provider1);
    assert!(result.is_err());
}

#[test]
fn test_submit_price_odd_providers() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, OraclePriceFeed);
    let client = OraclePriceFeedClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin, &300);

    let token_a = Address::generate(&env);
    let token_b = Address::generate(&env);
    let pair_id = Symbol::new(&env, "XLM_USDC");

    client.register_pair(&pair_id, &token_a, &token_b);

    let provider1 = Address::generate(&env);
    let provider2 = Address::generate(&env);
    let provider3 = Address::generate(&env);

    client.add_provider(&pair_id, &provider1);
    client.add_provider(&pair_id, &provider2);
    client.add_provider(&pair_id, &provider3);

    // Submit prices: 100, 200, 300 -> median should be 200
    client.submit_price(&pair_id, &provider1, &100);
    client.submit_price(&pair_id, &provider2, &200);
    client.submit_price(&pair_id, &provider3, &300);

    let (median, _timestamp) = client.get_price(&pair_id);
    assert_eq!(median, 200);
}

#[test]
fn test_submit_price_even_providers() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, OraclePriceFeed);
    let client = OraclePriceFeedClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin, &300);

    let token_a = Address::generate(&env);
    let token_b = Address::generate(&env);
    let pair_id = Symbol::new(&env, "XLM_USDC");

    client.register_pair(&pair_id, &token_a, &token_b);

    let provider1 = Address::generate(&env);
    let provider2 = Address::generate(&env);

    client.add_provider(&pair_id, &provider1);
    client.add_provider(&pair_id, &provider2);

    // Submit prices: 100, 200 -> median should be (100 + 200) / 2 = 150
    client.submit_price(&pair_id, &provider1, &100);
    client.submit_price(&pair_id, &provider2, &200);

    let (median, _timestamp) = client.get_price(&pair_id);
    assert_eq!(median, 150);
}

#[test]
fn test_unauthorized_provider() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, OraclePriceFeed);
    let client = OraclePriceFeedClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin, &300);

    let token_a = Address::generate(&env);
    let token_b = Address::generate(&env);
    let pair_id = Symbol::new(&env, "XLM_USDC");

    client.register_pair(&pair_id, &token_a, &token_b);

    let provider1 = Address::generate(&env);
    let unauthorized_provider = Address::generate(&env);

    client.add_provider(&pair_id, &provider1);

    // Try to submit price from unauthorized provider
    let result = client.try_submit_price(&pair_id, &unauthorized_provider, &100);
    assert!(result.is_err());
}

#[test]
fn test_invalid_price() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, OraclePriceFeed);
    let client = OraclePriceFeedClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin, &300);

    let token_a = Address::generate(&env);
    let token_b = Address::generate(&env);
    let pair_id = Symbol::new(&env, "XLM_USDC");

    client.register_pair(&pair_id, &token_a, &token_b);

    let provider1 = Address::generate(&env);

    client.add_provider(&pair_id, &provider1);

    // Try to submit invalid (zero or negative) price
    let result = client.try_submit_price(&pair_id, &provider1, &0);
    assert!(result.is_err());

    let result = client.try_submit_price(&pair_id, &provider1, &-100);
    assert!(result.is_err());
}

#[test]
fn test_stale_price() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, OraclePriceFeed);
    let client = OraclePriceFeedClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin, &300);

    let token_a = Address::generate(&env);
    let token_b = Address::generate(&env);
    let pair_id = Symbol::new(&env, "XLM_USDC");

    client.register_pair(&pair_id, &token_a, &token_b);

    let provider1 = Address::generate(&env);

    client.add_provider(&pair_id, &provider1);

    // Set initial time
    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    client.submit_price(&pair_id, &provider1, &100);

    // Advance time beyond stale threshold
    env.ledger().with_mut(|li| {
        li.timestamp = 2000; // 1000 + 300 + 700 = 2000 > 1300
    });

    // Try to get stale price
    let result = client.try_get_price(&pair_id);
    assert!(result.is_err());
}

#[test]
fn test_price_history() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, OraclePriceFeed);
    let client = OraclePriceFeedClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin, &300);

    let token_a = Address::generate(&env);
    let token_b = Address::generate(&env);
    let pair_id = Symbol::new(&env, "XLM_USDC");

    client.register_pair(&pair_id, &token_a, &token_b);

    let provider1 = Address::generate(&env);
    let provider2 = Address::generate(&env);

    client.add_provider(&pair_id, &provider1);
    client.add_provider(&pair_id, &provider2);

    // Submit multiple prices
    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });
    client.submit_price(&pair_id, &provider1, &100);
    client.submit_price(&pair_id, &provider2, &200);

    env.ledger().with_mut(|li| {
        li.timestamp = 2000;
    });
    client.submit_price(&pair_id, &provider1, &150);
    client.submit_price(&pair_id, &provider2, &250);

    // Get history
    let history = client.get_price_history(&pair_id, &10);
    assert_eq!(history.len(), 4);
}

#[test]
fn test_set_stale_threshold() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, OraclePriceFeed);
    let client = OraclePriceFeedClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin, &300);

    client.set_stale_threshold(&600);

    let token_a = Address::generate(&env);
    let token_b = Address::generate(&env);
    let pair_id = Symbol::new(&env, "XLM_USDC");

    client.register_pair(&pair_id, &token_a, &token_b);

    let provider1 = Address::generate(&env);

    client.add_provider(&pair_id, &provider1);

    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    client.submit_price(&pair_id, &provider1, &100);

    // Advance time to 1500 (within new threshold of 600)
    env.ledger().with_mut(|li| {
        li.timestamp = 1500;
    });

    // Should not be stale
    let (median, _timestamp) = client.get_price(&pair_id);
    assert_eq!(median, 100);
}
