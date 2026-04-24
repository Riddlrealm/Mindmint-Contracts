#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, String,
};

// ─────────────────────────────────────────────────────────
// HELPERS
// ─────────────────────────────────────────────────────────

fn create_token<'a>(env: &Env, admin: &Address) -> (Address, TokenClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let addr = sac.address();
    (addr.clone(), TokenClient::new(env, &addr))
}

struct TestEnv {
    env: Env,
    admin: Address,
    player: Address,
    contract_id: Address,
    token_id: Address,
}

impl TestEnv {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let player = Address::generate(&env);
        let token_admin = Address::generate(&env);

        let (token_id, _token) = create_token(&env, &token_admin);
        let asset_admin = StellarAssetClient::new(&env, &token_id);

        let contract_id = env.register_contract(None, PuzzleSubscriptionTierContract);
        let client = PuzzleSubscriptionTierContractClient::new(&env, &contract_id);
        client.initialize(&admin, &token_id);

        // Fund player
        asset_admin.mint(&player, &100_000);

        TestEnv { env, admin, player, contract_id, token_id }
    }

    fn client(&self) -> PuzzleSubscriptionTierContractClient<'_> {
        PuzzleSubscriptionTierContractClient::new(&self.env, &self.contract_id)
    }

    fn token(&self) -> TokenClient<'_> {
        TokenClient::new(&self.env, &self.token_id)
    }

    fn asset_admin(&self) -> StellarAssetClient<'_> {
        StellarAssetClient::new(&self.env, &self.token_id)
    }
}

fn register_tiers(t: &TestEnv) {
    let client = t.client();
    let empty: soroban_sdk::Vec<String> = vec![&t.env];

    client.set_tier_config(&t.admin, &Tier::Free, &0, &30, &1, &empty);

    client.set_tier_config(
        &t.admin,
        &Tier::Pro,
        &100,
        &30,
        &5,
        &vec![&t.env, String::from_str(&t.env, "hints")],
    );

    client.set_tier_config(
        &t.admin,
        &Tier::Elite,
        &200,
        &30,
        &10,
        &vec![
            &t.env,
            String::from_str(&t.env, "hints"),
            String::from_str(&t.env, "leaderboard"),
        ],
    );
}

// ─────────────────────────────────────────────────────────
// TESTS
// ─────────────────────────────────────────────────────────

#[test]
fn test_subscribe_free_tier() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let sub_id = client.subscribe(&t.player, &Tier::Free);
    assert_eq!(sub_id, 1);

    let sub = client.get_subscription(&sub_id);
    assert_eq!(sub.holder, t.player);
    assert_eq!(sub.tier, Tier::Free);
    assert!(sub.expires_at > sub.started_at);

    // No tokens charged for free tier.
    assert_eq!(t.token().balance(&t.player), 100_000);
}

#[test]
fn test_subscribe_pro_tier_charges_price() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let sub_id = client.subscribe(&t.player, &Tier::Pro);
    assert_eq!(sub_id, 1);
    assert_eq!(t.token().balance(&t.player), 100_000 - 100);

    let sub = client.get_subscription(&sub_id);
    assert_eq!(sub.tier, Tier::Pro);
}

#[test]
fn test_subscribe_elite_tier() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let sub_id = client.subscribe(&t.player, &Tier::Elite);
    assert_eq!(t.token().balance(&t.player), 100_000 - 200);

    let sub = client.get_subscription(&sub_id);
    assert_eq!(sub.tier, Tier::Elite);
}

#[test]
#[should_panic(expected = "existing subscription still active")]
fn test_subscribe_fails_when_active_subscription_exists() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    client.subscribe(&t.player, &Tier::Pro);
    // Second subscription should panic.
    client.subscribe(&t.player, &Tier::Free);
}

#[test]
fn test_subscribe_allowed_after_expiry() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    client.subscribe(&t.player, &Tier::Pro);

    // Advance time past expiry (30 s in test mode).
    t.env.ledger().set_timestamp(t.env.ledger().timestamp() + 31);

    // Should succeed since previous subscription is expired.
    let sub_id2 = client.subscribe(&t.player, &Tier::Free);
    assert!(sub_id2 > 0);
}

#[test]
fn test_renew_extends_expiry() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let sub_id = client.subscribe(&t.player, &Tier::Pro);
    let sub_before = client.get_subscription(&sub_id);

    // Enable auto-renew.
    client.set_auto_renew(&sub_id, &true);

    // Advance time (still within period).
    t.env.ledger().set_timestamp(t.env.ledger().timestamp() + 10);

    client.renew(&t.player, &sub_id);

    let sub_after = client.get_subscription(&sub_id);
    assert!(sub_after.expires_at > sub_before.expires_at);
    // An extra 100 charged.
    assert_eq!(t.token().balance(&t.player), 100_000 - 100 - 100);
}

#[test]
fn test_renew_without_auto_renew_requires_holder() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    // auto_renew defaults to false.
    let sub_id = client.subscribe(&t.player, &Tier::Pro);

    // Holder can renew themselves.
    client.renew(&t.player, &sub_id);
    let sub = client.get_subscription(&sub_id);
    assert!(sub.expires_at > t.env.ledger().timestamp());
}

#[test]
fn test_upgrade_proration() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    // Subscribe to Pro at t=0; period = 30 s, price = 100.
    let sub_id = client.subscribe(&t.player, &Tier::Pro);
    let balance_after_sub = t.token().balance(&t.player);

    // Advance to the midpoint (15 s remaining).
    t.env.ledger().set_timestamp(t.env.ledger().timestamp() + 15);

    // Upgrade to Elite (price = 200).
    // remaining_value = 100 * 15 / 30 = 50
    // charge = 200 - 50 = 150
    client.upgrade(&sub_id, &Tier::Elite);

    let sub = client.get_subscription(&sub_id);
    assert_eq!(sub.tier, Tier::Elite);

    let charged = balance_after_sub - t.token().balance(&t.player);
    assert_eq!(charged, 150);

    // Period reset to full 30 s from upgrade time.
    let now = t.env.ledger().timestamp();
    assert_eq!(sub.expires_at, now + 30);
}

#[test]
fn test_upgrade_zero_charge_when_remaining_exceeds_new_price() {
    // Edge case: if remaining_value >= new_price, charge = 0.
    let t = TestEnv::new();
    let client = t.client();
    let empty: soroban_sdk::Vec<String> = vec![&t.env];

    // Pro: price=1000, duration=30 days
    client.set_tier_config(&t.admin, &Tier::Pro, &1000, &30, &5, &empty);
    // Elite: price=100, duration=30 days (cheaper — tests the floor)
    client.set_tier_config(&t.admin, &Tier::Elite, &100, &30, &10, &empty);

    // Give player enough for Pro.
    t.asset_admin().mint(&t.player, &900); // now has 100_900 total

    let sub_id = client.subscribe(&t.player, &Tier::Pro);

    // Advance only 1 second; remaining_value = 1000 * 29 / 30 ≈ 966 > 100
    t.env.ledger().set_timestamp(t.env.ledger().timestamp() + 1);

    let balance_before_upgrade = t.token().balance(&t.player);
    client.upgrade(&sub_id, &Tier::Elite);

    // Charge capped at 0.
    assert_eq!(t.token().balance(&t.player), balance_before_upgrade);
}

#[test]
#[should_panic(expected = "can only upgrade to a higher tier")]
fn test_upgrade_rejects_downgrade() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let sub_id = client.subscribe(&t.player, &Tier::Elite);
    client.upgrade(&sub_id, &Tier::Pro);
}

#[test]
#[should_panic(expected = "subscription has expired")]
fn test_upgrade_rejects_expired_subscription() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let sub_id = client.subscribe(&t.player, &Tier::Pro);

    t.env.ledger().set_timestamp(t.env.ledger().timestamp() + 31);

    client.upgrade(&sub_id, &Tier::Elite);
}

#[test]
fn test_cancel_disables_auto_renew() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let sub_id = client.subscribe(&t.player, &Tier::Pro);
    client.set_auto_renew(&sub_id, &true);

    client.cancel(&sub_id);

    let sub = client.get_subscription(&sub_id);
    assert!(!sub.auto_renew);
    // Subscription is still active until expires_at.
    assert!(sub.expires_at > t.env.ledger().timestamp());
}

#[test]
fn test_has_access_active_subscription() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    client.subscribe(&t.player, &Tier::Pro);

    assert!(client.has_access(&t.player, &Tier::Free));
    assert!(client.has_access(&t.player, &Tier::Pro));
    assert!(!client.has_access(&t.player, &Tier::Elite));
}

#[test]
fn test_has_access_elite_passes_all_tiers() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    client.subscribe(&t.player, &Tier::Elite);

    assert!(client.has_access(&t.player, &Tier::Free));
    assert!(client.has_access(&t.player, &Tier::Pro));
    assert!(client.has_access(&t.player, &Tier::Elite));
}

#[test]
fn test_has_access_returns_false_for_expired_subscription() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    client.subscribe(&t.player, &Tier::Pro);

    // Advance past expiry.
    t.env.ledger().set_timestamp(t.env.ledger().timestamp() + 31);

    assert!(!client.has_access(&t.player, &Tier::Pro));
    assert!(!client.has_access(&t.player, &Tier::Elite));
}

#[test]
fn test_has_access_no_subscription_only_free() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    // No subscription at all.
    assert!(client.has_access(&t.player, &Tier::Free));
    assert!(!client.has_access(&t.player, &Tier::Pro));
    assert!(!client.has_access(&t.player, &Tier::Elite));
}

#[test]
fn test_tier_config_update() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let empty: soroban_sdk::Vec<String> = vec![&t.env];

    // Update Pro tier price.
    client.set_tier_config(&t.admin, &Tier::Pro, &500, &60, &7, &empty);

    let cfg = client.get_tier_config(&Tier::Pro);
    assert_eq!(cfg.price, 500);
    assert_eq!(cfg.duration_days, 60);
    assert_eq!(cfg.puzzle_access_level, 7);
}

#[test]
fn test_subscription_id_increments() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let player2 = Address::generate(&t.env);
    let player3 = Address::generate(&t.env);
    t.asset_admin().mint(&player2, &100_000);
    t.asset_admin().mint(&player3, &100_000);

    let id1 = client.subscribe(&player2, &Tier::Free);
    let id2 = client.subscribe(&player3, &Tier::Free);

    assert_eq!(id2, id1 + 1);
}

#[test]
fn test_get_player_subscription_id() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let unrelated = Address::generate(&t.env);

    let sub_id = client.subscribe(&t.player, &Tier::Pro);

    assert_eq!(client.get_player_subscription_id(&t.player), Some(sub_id));
    assert_eq!(client.get_player_subscription_id(&unrelated), None);
}
