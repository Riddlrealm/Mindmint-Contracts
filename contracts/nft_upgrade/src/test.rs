#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, Map, String,
};

// ──────────────────────────────────────────────────────────
// HELPERS
// ──────────────────────────────────────────────────────────

fn create_token<'a>(env: &'a Env, admin: &Address) -> (Address, TokenClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let addr = sac.address();
    (addr.clone(), TokenClient::new(env, &addr))
}

fn mint_tokens(env: &Env, token_addr: &Address, to: &Address, amount: i128) {
    StellarAssetClient::new(env, token_addr).mint(to, &amount);
}

/// Registers the upgrade contract and initialises it.
/// Uses a random address as the NFT contract (no real wasm needed because the
/// ownership cross-contract call uses `try_invoke_contract` and fails gracefully).
fn setup(env: &Env) -> (NftUpgradeContractClient, Address, Address, Address, TokenClient, Address) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let player = Address::generate(env);
    let token_admin = Address::generate(env);
    let (token_addr, token_client) = create_token(env, &token_admin);
    let nft_contract = Address::generate(env); // stub — owner_of call will fail gracefully

    let contract_id = env.register_contract(None, NftUpgradeContract);
    let client = NftUpgradeContractClient::new(env, &contract_id);
    client.initialize(&admin, &token_addr, &nft_contract);

    mint_tokens(env, &token_addr, &player, 100_000);

    (client, admin, player, token_addr, token_client, nft_contract)
}

fn boosts(env: &Env, power: u32, rarity: u32) -> Map<String, u32> {
    let mut m = Map::new(env);
    m.set(String::from_str(env, "power_level"), power);
    m.set(String::from_str(env, "rarity_tier"), rarity);
    m
}

/// Registers the contract + a tier config, funds the player.
fn setup_with_tier(
    env: &Env,
    tier: u32,
    cost: i128,
    success_rate_bps: u32,
) -> (NftUpgradeContractClient, Address, Address, Address) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let player = Address::generate(env);
    let token_admin = Address::generate(env);
    let (token_addr, _) = create_token(env, &token_admin);
    let nft_contract = Address::generate(env);

    let contract_id = env.register_contract(None, NftUpgradeContract);
    let client = NftUpgradeContractClient::new(env, &contract_id);
    client.initialize(&admin, &token_addr, &nft_contract);

    let b = boosts(env, 20, 2);
    client.register_upgrade_config(&tier, &cost, &success_rate_bps, &b);

    mint_tokens(env, &token_addr, &player, 100_000);

    (client, admin, player, token_addr)
}

// ──────────────────────────────────────────────────────────
// INITIALISATION
// ──────────────────────────────────────────────────────────

#[test]
fn test_initialize_success() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token_addr, _) = create_token(&env, &token_admin);
    let nft_contract = Address::generate(&env);

    let contract_id = env.register_contract(None, NftUpgradeContract);
    let client = NftUpgradeContractClient::new(&env, &contract_id);
    client.initialize(&admin, &token_addr, &nft_contract);

    let config = client.get_contract_config_pub();
    assert_eq!(config.admin, admin);
    assert_eq!(config.token, token_addr);
    assert_eq!(config.nft_contract, nft_contract);
}

#[test]
#[should_panic(expected = "AlreadyInitialized")]
fn test_double_initialize_panics() {
    let env = Env::default();
    let (client, admin, _, token_addr, _, nft_contract) = setup(&env);
    client.initialize(&admin, &token_addr, &nft_contract);
}

// ──────────────────────────────────────────────────────────
// CONFIG MANAGEMENT
// ──────────────────────────────────────────────────────────

#[test]
fn test_register_and_get_config() {
    let env = Env::default();
    let (client, _, _, _, _, _) = setup(&env);

    let b = boosts(&env, 10, 1);
    client.register_upgrade_config(&1u32, &500i128, &7500u32, &b);

    let cfg = client.get_config(&1u32);
    assert_eq!(cfg.tier, 1);
    assert_eq!(cfg.cost, 500);
    assert_eq!(cfg.success_rate_bps, 7500);
    assert_eq!(cfg.attribute_boosts.get(String::from_str(&env, "power_level")), Some(10));
}

#[test]
#[should_panic(expected = "TierNotFound")]
fn test_get_config_missing_tier_panics() {
    let env = Env::default();
    let (client, _, _, _, _, _) = setup(&env);
    client.get_config(&99u32);
}

#[test]
#[should_panic(expected = "MaxTierReached")]
fn test_register_config_beyond_max_tier_panics() {
    let env = Env::default();
    let (client, _, _, _, _, _) = setup(&env);
    let b = boosts(&env, 5, 1);
    client.register_upgrade_config(&6u32, &100i128, &5000u32, &b);
}

#[test]
fn test_success_rate_clamped_to_bps_denominator() {
    let env = Env::default();
    let (client, _, _, _, _, _) = setup(&env);
    let b = boosts(&env, 1, 0);
    client.register_upgrade_config(&1u32, &100i128, &15_000u32, &b);
    let cfg = client.get_config(&1u32);
    assert_eq!(cfg.success_rate_bps, 10_000);
}

// ──────────────────────────────────────────────────────────
// UPGRADE ATTEMPT: SUCCESS PATH
// ──────────────────────────────────────────────────────────

#[test]
fn test_successful_upgrade_applies_attribute_boosts() {
    let env = Env::default();
    // 100 % success rate → always succeeds regardless of PRNG roll
    let (client, _, player, _) = setup_with_tier(&env, 1, 500, 10_000);
    env.ledger().with_mut(|l| l.timestamp = 1_000);

    let nft_id: u32 = 1;
    let success = client.attempt_upgrade(&player, &nft_id, &1u32);
    assert!(success);

    let attrs = client.get_nft_attributes(&nft_id);
    assert_eq!(attrs.get(String::from_str(&env, "power_level")), Some(20));
    assert_eq!(attrs.get(String::from_str(&env, "rarity_tier")), Some(2));
}

#[test]
fn test_successful_upgrade_records_history() {
    let env = Env::default();
    let (client, _, player, _) = setup_with_tier(&env, 1, 500, 10_000);
    env.ledger().with_mut(|l| l.timestamp = 2_000);

    let nft_id: u32 = 42;
    client.attempt_upgrade(&player, &nft_id, &1u32);

    let history = client.get_upgrade_history(&nft_id);
    assert_eq!(history.len(), 1);
    let attempt = history.get(0).unwrap();
    assert_eq!(attempt.nft_id, nft_id);
    assert!(attempt.success);
    assert_eq!(attempt.tokens_spent, 500);
    assert_eq!(attempt.attempted_at, 2_000);
}

#[test]
fn test_cost_deducted_upfront_on_success() {
    let env = Env::default();
    let cost: i128 = 1_000;
    let (client, _, player, token_addr) = setup_with_tier(&env, 1, cost, 10_000);
    let token_client = TokenClient::new(&env, &token_addr);
    let before = token_client.balance(&player);

    client.attempt_upgrade(&player, &1u32, &1u32);

    let after = token_client.balance(&player);
    assert_eq!(before - after, cost);
}

#[test]
fn test_successful_upgrade_accumulates_attribute_boosts() {
    let env = Env::default();
    let (client, _, player, _) = setup_with_tier(&env, 1, 100, 10_000);

    let nft_id: u32 = 9;
    client.attempt_upgrade(&player, &nft_id, &1u32);
    client.attempt_upgrade(&player, &nft_id, &1u32);

    let attrs = client.get_nft_attributes(&nft_id);
    assert_eq!(attrs.get(String::from_str(&env, "power_level")), Some(40));
    assert_eq!(attrs.get(String::from_str(&env, "rarity_tier")), Some(4));
}

// ──────────────────────────────────────────────────────────
// UPGRADE ATTEMPT: FAILURE PATH
// ──────────────────────────────────────────────────────────

#[test]
fn test_failed_upgrade_refunds_50_percent() {
    let env = Env::default();
    // 0 % success rate → always fails
    let (client, _, player, token_addr) = setup_with_tier(&env, 1, 500, 0);
    let token_client = TokenClient::new(&env, &token_addr);
    let before = token_client.balance(&player);

    let success = client.attempt_upgrade(&player, &1u32, &1u32);
    assert!(!success);

    let after = token_client.balance(&player);
    // Net deduction = 500 cost − 250 refund = 250
    assert_eq!(before - after, 250);
}

#[test]
fn test_failed_upgrade_records_history_with_unchanged_attributes() {
    let env = Env::default();
    let (client, _, player, _) = setup_with_tier(&env, 1, 500, 0);

    let nft_id: u32 = 3;
    client.attempt_upgrade(&player, &nft_id, &1u32);

    let history = client.get_upgrade_history(&nft_id);
    assert_eq!(history.len(), 1);
    let attempt = history.get(0).unwrap();
    assert!(!attempt.success);
    // No prior successful upgrade → attributes remain empty
    assert_eq!(attempt.new_attributes.len(), 0);
}

// ──────────────────────────────────────────────────────────
// UPGRADE ATTEMPT: VALIDATION / EDGE CASES
// ──────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "TierNotFound")]
fn test_upgrade_with_unregistered_tier_panics() {
    let env = Env::default();
    let (client, _, player, _) = setup_with_tier(&env, 1, 500, 5000);
    // Tier 2 config was never registered
    client.attempt_upgrade(&player, &1u32, &2u32);
}

#[test]
#[should_panic(expected = "MaxTierReached")]
fn test_upgrade_beyond_max_tier_panics() {
    let env = Env::default();
    let (client, _, player, _) = setup_with_tier(&env, 1, 500, 5000);
    client.attempt_upgrade(&player, &1u32, &6u32);
}

#[test]
fn test_multiple_upgrades_accumulate_history() {
    let env = Env::default();
    let (client, _, player, _) = setup_with_tier(&env, 1, 100, 10_000);

    let nft_id: u32 = 5;
    client.attempt_upgrade(&player, &nft_id, &1u32);
    client.attempt_upgrade(&player, &nft_id, &1u32);
    client.attempt_upgrade(&player, &nft_id, &1u32);

    let history = client.get_upgrade_history(&nft_id);
    assert_eq!(history.len(), 3);
}

// ──────────────────────────────────────────────────────────
// QUERIES: EMPTY STATE
// ──────────────────────────────────────────────────────────

#[test]
fn test_get_upgrade_history_empty_for_new_nft() {
    let env = Env::default();
    let (client, _, _, _, _, _) = setup(&env);
    assert_eq!(client.get_upgrade_history(&999u32).len(), 0);
}

#[test]
fn test_get_nft_attributes_empty_for_new_nft() {
    let env = Env::default();
    let (client, _, _, _, _, _) = setup(&env);
    assert_eq!(client.get_nft_attributes(&999u32).len(), 0);
}
