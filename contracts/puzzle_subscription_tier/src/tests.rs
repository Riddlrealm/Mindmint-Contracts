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

        // Fund player with plenty of tokens.
        asset_admin.mint(&player, &1_000_000);

        TestEnv {
            env,
            admin,
            player,
            contract_id,
            token_id,
        }
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

/// Register all three tiers with default test values.
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
// BASIC SUBSCRIBE TESTS
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
    assert_eq!(t.token().balance(&t.player), 1_000_000);

    // Locked price should match tier config at subscribe time.
    assert_eq!(sub.locked_price, 0);
    assert_eq!(sub.locked_duration_days, 30);
}

#[test]
fn test_subscribe_pro_tier_charges_price() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let sub_id = client.subscribe(&t.player, &Tier::Pro);
    assert_eq!(sub_id, 1);
    assert_eq!(t.token().balance(&t.player), 1_000_000 - 100);

    let sub = client.get_subscription(&sub_id);
    assert_eq!(sub.tier, Tier::Pro);
    assert_eq!(sub.locked_price, 100);
    assert_eq!(sub.locked_duration_days, 30);
}

#[test]
fn test_subscribe_elite_tier() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let sub_id = client.subscribe(&t.player, &Tier::Elite);
    assert_eq!(t.token().balance(&t.player), 1_000_000 - 200);

    let sub = client.get_subscription(&sub_id);
    assert_eq!(sub.tier, Tier::Elite);
    assert_eq!(sub.locked_price, 200);
    assert_eq!(sub.locked_duration_days, 30);
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
fn test_subscription_id_increments() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let player2 = Address::generate(&t.env);
    let player3 = Address::generate(&t.env);
    t.asset_admin().mint(&player2, &1_000_000);
    t.asset_admin().mint(&player3, &1_000_000);

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

// ─────────────────────────────────────────────────────────
// RENEW TESTS
// ─────────────────────────────────────────────────────────

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
    assert_eq!(t.token().balance(&t.player), 1_000_000 - 100 - 100);
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

// ─────────────────────────────────────────────────────────
// UPGRADE TESTS
// ─────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────
// CANCEL TESTS
// ─────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────
// ACCESS GATE TESTS
// ─────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────
// TIER CONFIG TESTS
// ─────────────────────────────────────────────────────────

#[test]
fn test_tier_config_update() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let empty: soroban_sdk::Vec<String> = vec![&t.env];

    // Update Pro tier price using set_tier_config.
    client.set_tier_config(&t.admin, &Tier::Pro, &500, &60, &7, &empty);

    let cfg = client.get_tier_config(&Tier::Pro);
    assert_eq!(cfg.price, 500);
    assert_eq!(cfg.duration_days, 60);
    assert_eq!(cfg.puzzle_access_level, 7);
}

// ─────────────────────────────────────────────────────────
// DYNAMIC PRICING — update_tier_price
// ─────────────────────────────────────────────────────────

/// After `update_tier_price`, the live TierConfig reflects the new price.
#[test]
fn test_update_tier_price_changes_live_config() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    client.update_tier_price(&t.admin, &Tier::Pro, &250, &30);

    let cfg = client.get_tier_config(&Tier::Pro);
    assert_eq!(cfg.price, 250);
    assert_eq!(cfg.duration_days, 30);
}

/// `update_tier_price` also changes the duration when a new one is supplied.
#[test]
fn test_update_tier_price_also_changes_duration() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    client.update_tier_price(&t.admin, &Tier::Elite, &300, &45);

    let cfg = client.get_tier_config(&Tier::Elite);
    assert_eq!(cfg.price, 300);
    assert_eq!(cfg.duration_days, 45);
}

/// A new subscriber after the price change pays the new price.
#[test]
fn test_new_subscriber_pays_updated_price() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    // Raise Pro from 100 → 250.
    client.update_tier_price(&t.admin, &Tier::Pro, &250, &30);

    let player2 = Address::generate(&t.env);
    t.asset_admin().mint(&player2, &1_000_000);

    let sub_id = client.subscribe(&player2, &Tier::Pro);
    assert_eq!(t.token().balance(&player2), 1_000_000 - 250);

    let sub = client.get_subscription(&sub_id);
    assert_eq!(sub.locked_price, 250);
}

// ─────────────────────────────────────────────────────────
// GRANDFATHERING — existing subscriber keeps old price on renewal
// ─────────────────────────────────────────────────────────

/// An active subscriber renews at their *locked* price, not the new price.
#[test]
fn test_existing_subscriber_renews_at_locked_price_after_price_increase() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    // Player subscribes at Pro = 100.
    let sub_id = client.subscribe(&t.player, &Tier::Pro);
    let balance_after_sub = t.token().balance(&t.player);

    // Admin raises price to 500 – should NOT affect this subscriber's renewal.
    client.update_tier_price(&t.admin, &Tier::Pro, &500, &30);

    // Renew: should charge 100, not 500.
    client.renew(&t.player, &sub_id);

    let renewal_cost = balance_after_sub - t.token().balance(&t.player);
    assert_eq!(renewal_cost, 100, "renewal should use grandfathered price 100, not 500");
}

/// Locked price is preserved through multiple admin price changes.
#[test]
fn test_locked_price_survives_multiple_price_changes() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let sub_id = client.subscribe(&t.player, &Tier::Pro);
    let balance_after_sub = t.token().balance(&t.player);

    // Two successive price changes.
    client.update_tier_price(&t.admin, &Tier::Pro, &300, &30);
    client.update_tier_price(&t.admin, &Tier::Pro, &999, &30);

    // Still renews at the original 100.
    client.renew(&t.player, &sub_id);

    let renewal_cost = balance_after_sub - t.token().balance(&t.player);
    assert_eq!(renewal_cost, 100);
}

/// A price *decrease* is also not applied retroactively to existing subscribers.
#[test]
fn test_existing_subscriber_renews_at_locked_price_after_price_decrease() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    // Player subscribes at Elite = 200.
    let sub_id = client.subscribe(&t.player, &Tier::Elite);
    let balance_after_sub = t.token().balance(&t.player);

    // Admin drops Elite to 50 – grandfathered subscriber should still pay 200
    // (they agreed to 200; they retain the features they paid for).
    client.update_tier_price(&t.admin, &Tier::Elite, &50, &30);

    client.renew(&t.player, &sub_id);

    let renewal_cost = balance_after_sub - t.token().balance(&t.player);
    assert_eq!(renewal_cost, 200, "grandfathered price 200 should be used, not the new 50");
}

/// After subscription expiry and re-subscribe, the scout pays the new (current) price.
#[test]
fn test_resubscribe_after_expiry_picks_up_new_price() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    // Subscribe at 100 then let it expire.
    client.subscribe(&t.player, &Tier::Pro);
    t.env.ledger().set_timestamp(t.env.ledger().timestamp() + 31);

    // Admin raises price while subscriber is lapsed.
    client.update_tier_price(&t.admin, &Tier::Pro, &400, &30);

    let balance_before_resub = t.token().balance(&t.player);
    let sub_id2 = client.subscribe(&t.player, &Tier::Pro);

    let resub_cost = balance_before_resub - t.token().balance(&t.player);
    assert_eq!(resub_cost, 400, "new subscription must use the current price 400");

    let sub = client.get_subscription(&sub_id2);
    assert_eq!(sub.locked_price, 400);
}

// ─────────────────────────────────────────────────────────
// UPGRADE AFTER PRICE CHANGE
// ─────────────────────────────────────────────────────────

/// When upgrading, the new tier's CURRENT (live) price is used for the upgrade charge,
/// and the resulting locked_price reflects that current price.
#[test]
fn test_upgrade_uses_current_price_for_new_tier() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    // Subscribe to Pro at 100.
    let sub_id = client.subscribe(&t.player, &Tier::Pro);
    let balance_after_sub = t.token().balance(&t.player);

    // Admin raises Elite price from 200 → 350 before the upgrade.
    client.update_tier_price(&t.admin, &Tier::Elite, &350, &30);

    // Advance to midpoint: remaining_value = 100 * 15 / 30 = 50
    // charge = 350 - 50 = 300
    t.env.ledger().set_timestamp(t.env.ledger().timestamp() + 15);
    client.upgrade(&sub_id, &Tier::Elite);

    let upgrade_cost = balance_after_sub - t.token().balance(&t.player);
    assert_eq!(upgrade_cost, 300);

    // After upgrade, locked_price should be the current Elite price (350).
    let sub = client.get_subscription(&sub_id);
    assert_eq!(sub.locked_price, 350);
    assert_eq!(sub.tier, Tier::Elite);
}

/// After an upgrade, future renewals use the price locked at upgrade time.
#[test]
fn test_post_upgrade_renewal_uses_upgrade_locked_price() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let sub_id = client.subscribe(&t.player, &Tier::Pro);

    // Advance and upgrade to Elite at current price 200.
    t.env.ledger().set_timestamp(t.env.ledger().timestamp() + 15);
    client.upgrade(&sub_id, &Tier::Elite);

    let balance_after_upgrade = t.token().balance(&t.player);

    // Admin now changes Elite to 500 – subscriber's renewal should stay at 200.
    client.update_tier_price(&t.admin, &Tier::Elite, &500, &30);

    client.renew(&t.player, &sub_id);

    let renewal_cost = balance_after_upgrade - t.token().balance(&t.player);
    assert_eq!(renewal_cost, 200, "post-upgrade locked price 200 should be used");
}

// ─────────────────────────────────────────────────────────
// PRICE HISTORY / AUDIT LOG TESTS
// ─────────────────────────────────────────────────────────

/// With no price changes, get_price_history returns an empty vec.
#[test]
fn test_price_history_empty_before_any_changes() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let history = client.get_price_history(&Tier::Pro);
    assert_eq!(history.len(), 0);
}

/// A single update_tier_price call produces exactly one audit record.
#[test]
fn test_price_history_records_single_change() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    let ts_before = t.env.ledger().timestamp();
    client.update_tier_price(&t.admin, &Tier::Pro, &250, &30);

    let history = client.get_price_history(&Tier::Pro);
    assert_eq!(history.len(), 1);

    let record = history.get(0).unwrap();
    assert_eq!(record.old_price, 100);
    assert_eq!(record.new_price, 250);
    assert_eq!(record.old_duration_days, 30);
    assert_eq!(record.new_duration_days, 30);
    assert_eq!(record.changed_by, t.admin);
    assert!(record.changed_at >= ts_before);
    assert_eq!(record.tier as u32, Tier::Pro as u32);
}

/// Multiple successive changes all appear in the log.
#[test]
fn test_price_history_records_multiple_changes() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    client.update_tier_price(&t.admin, &Tier::Pro, &150, &30);
    client.update_tier_price(&t.admin, &Tier::Pro, &200, &45);
    client.update_tier_price(&t.admin, &Tier::Pro, &175, &45);

    let history = client.get_price_history(&Tier::Pro);
    assert_eq!(history.len(), 3);

    // Verify the chain of old→new prices.
    assert_eq!(history.get(0).unwrap().old_price, 100);
    assert_eq!(history.get(0).unwrap().new_price, 150);

    assert_eq!(history.get(1).unwrap().old_price, 150);
    assert_eq!(history.get(1).unwrap().new_price, 200);

    assert_eq!(history.get(2).unwrap().old_price, 200);
    assert_eq!(history.get(2).unwrap().new_price, 175);
}

/// Price history for one tier is independent of another tier's history.
#[test]
fn test_price_history_is_tier_isolated() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    client.update_tier_price(&t.admin, &Tier::Pro, &150, &30);

    let elite_history = client.get_price_history(&Tier::Elite);
    assert_eq!(elite_history.len(), 0, "Elite history should be empty when only Pro changed");

    let pro_history = client.get_price_history(&Tier::Pro);
    assert_eq!(pro_history.len(), 1);
}

/// Duration changes are also captured in the audit record.
#[test]
fn test_price_history_captures_duration_change() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    client.update_tier_price(&t.admin, &Tier::Elite, &200, &60);

    let history = client.get_price_history(&Tier::Elite);
    let record = history.get(0).unwrap();
    assert_eq!(record.old_duration_days, 30);
    assert_eq!(record.new_duration_days, 60);
}

// ─────────────────────────────────────────────────────────
// UPDATE_TIER_PRICE — INPUT VALIDATION
// ─────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "tier not configured")]
fn test_update_tier_price_panics_if_tier_not_configured() {
    let t = TestEnv::new();
    // Do NOT call register_tiers — tier has no config yet.
    let client = t.client();
    client.update_tier_price(&t.admin, &Tier::Pro, &100, &30);
}

#[test]
#[should_panic(expected = "new_price must be non-negative")]
fn test_update_tier_price_rejects_negative_price() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();
    client.update_tier_price(&t.admin, &Tier::Pro, &-1, &30);
}

#[test]
#[should_panic(expected = "new_duration_days must be > 0")]
fn test_update_tier_price_rejects_zero_duration() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();
    client.update_tier_price(&t.admin, &Tier::Pro, &100, &0);
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_update_tier_price_rejects_non_admin() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();
    let impostor = Address::generate(&t.env);
    client.update_tier_price(&impostor, &Tier::Pro, &100, &30);
}

// ─────────────────────────────────────────────────────────
// LOCKED DURATION — GRANDFATHERED PERIOD LENGTH
// ─────────────────────────────────────────────────────────

/// Changing the duration does not change an existing subscriber's period length on renewal.
#[test]
fn test_existing_subscriber_renews_at_locked_duration() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    // Subscribe when Pro duration = 30 s (in test).
    let sub_id = client.subscribe(&t.player, &Tier::Pro);

    // Admin changes duration to 60 s.
    client.update_tier_price(&t.admin, &Tier::Pro, &100, &60);

    // Renew while still active.
    t.env.ledger().set_timestamp(t.env.ledger().timestamp() + 5);
    let sub_before_renew = client.get_subscription(&sub_id);
    client.renew(&t.player, &sub_id);

    let sub_after = client.get_subscription(&sub_id);
    // Extension should be 30 (locked), not 60 (new config).
    let extension = sub_after.expires_at - sub_before_renew.expires_at;
    assert_eq!(extension, 30 * SECONDS_PER_DAY, "should use locked duration 30, not new 60");
}

/// A brand-new subscriber after the duration change gets the new 60-day period.
#[test]
fn test_new_subscriber_gets_updated_duration() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    client.update_tier_price(&t.admin, &Tier::Pro, &100, &60);

    let player2 = Address::generate(&t.env);
    t.asset_admin().mint(&player2, &1_000_000);

    let sub_id = client.subscribe(&player2, &Tier::Pro);
    let sub = client.get_subscription(&sub_id);

    // Duration should be 60 s in test mode.
    assert_eq!(sub.locked_duration_days, 60);
    assert_eq!(sub.expires_at - sub.started_at, 60 * SECONDS_PER_DAY);
}

// ─────────────────────────────────────────────────────────
// ZERO-PRICE TIER EDGE CASES
// ─────────────────────────────────────────────────────────

/// Updating a zero-price free tier to a paid tier works and is recorded in history.
#[test]
fn test_update_free_tier_to_paid() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    client.update_tier_price(&t.admin, &Tier::Free, &10, &30);

    let cfg = client.get_tier_config(&Tier::Free);
    assert_eq!(cfg.price, 10);

    let history = client.get_price_history(&Tier::Free);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0).unwrap().old_price, 0);
    assert_eq!(history.get(0).unwrap().new_price, 10);
}

// ─────────────────────────────────────────────────────────
// MULTIPLE PLAYERS — independent grandfathering
// ─────────────────────────────────────────────────────────

/// Two players subscribe at different prices; each renews at their own locked rate.
#[test]
fn test_two_players_independent_grandfathered_prices() {
    let t = TestEnv::new();
    register_tiers(&t);
    let client = t.client();

    // Player A subscribes at Pro = 100.
    let sub_a = client.subscribe(&t.player, &Tier::Pro);

    // Admin raises price to 400.
    client.update_tier_price(&t.admin, &Tier::Pro, &400, &30);

    // Player B subscribes at Pro = 400.
    let player_b = Address::generate(&t.env);
    t.asset_admin().mint(&player_b, &1_000_000);
    let sub_b = client.subscribe(&player_b, &Tier::Pro);

    let balance_a = t.token().balance(&t.player);
    let balance_b = t.token().balance(&player_b);

    // Both renew.
    client.renew(&t.player, &sub_a);
    client.renew(&player_b, &sub_b);

    assert_eq!(balance_a - t.token().balance(&t.player), 100, "Player A keeps grandfathered 100");
    assert_eq!(balance_b - t.token().balance(&player_b), 400, "Player B pays current 400");
}
