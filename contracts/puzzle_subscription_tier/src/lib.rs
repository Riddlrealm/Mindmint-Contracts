#![no_std]

mod tests;

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, Symbol, Vec, String,
};

// ─────────────────────────────────────────────────────────
// TIME CONSTANTS
// ─────────────────────────────────────────────────────────

#[cfg(not(test))]
const SECONDS_PER_DAY: u64 = 86_400;
#[cfg(test)]
const SECONDS_PER_DAY: u64 = 1;

// ─────────────────────────────────────────────────────────
// TIERS
// ─────────────────────────────────────────────────────────

/// Subscription tier levels. Higher numeric value means higher access.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum Tier {
    Free = 0,
    Pro = 1,
    Elite = 2,
}

// ─────────────────────────────────────────────────────────
// DATA KEYS
// ─────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Contract-wide admin config
    Admin,
    /// Payment token address
    PaymentToken,
    /// TierConfig keyed by Tier enum value
    TierConfig(Tier),
    /// Subscription record keyed by subscription id
    Subscription(u64),
    /// Maps player Address → their current subscription id
    PlayerSub(Address),
    /// Monotonic counter for subscription ids
    NextId,
}

// ─────────────────────────────────────────────────────────
// STRUCTS
// ─────────────────────────────────────────────────────────

/// Per-subscription state stored on-chain.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Subscription {
    /// The subscriber's address.
    pub holder: Address,
    /// Active tier level.
    pub tier: Tier,
    /// Ledger timestamp when the subscription was originally created.
    pub started_at: u64,
    /// Ledger timestamp at which this subscription expires.
    pub expires_at: u64,
    /// When true, anyone may call `renew` to extend the subscription.
    pub auto_renew: bool,
}

/// Configuration for a single tier, set by the admin.
#[contracttype]
#[derive(Clone, Debug)]
pub struct TierConfig {
    /// The tier this config applies to.
    pub tier: Tier,
    /// Token amount required to subscribe (or pay difference on upgrade).
    pub price: i128,
    /// How many days each subscription period lasts.
    pub duration_days: u64,
    /// Numeric access level exposed to other contracts.
    pub puzzle_access_level: u32,
    /// Arbitrary feature flag strings (e.g. "hints", "leaderboard").
    pub feature_flags: Vec<String>,
}

// ─────────────────────────────────────────────────────────
// EVENTS  (topic symbols)
// ─────────────────────────────────────────────────────────

const EVT_SUBSCRIBED: &str = "subscribed";
const EVT_RENEWED: &str = "renewed";
const EVT_UPGRADED: &str = "upgraded";
const EVT_CANCELLED: &str = "cancelled";

// ─────────────────────────────────────────────────────────
// CONTRACT
// ─────────────────────────────────────────────────────────

#[contract]
pub struct PuzzleSubscriptionTierContract;

#[contractimpl]
impl PuzzleSubscriptionTierContract {
    // ──────────────── INITIALIZATION ────────────────

    /// Initialize the contract.  Must be called once before any other function.
    pub fn initialize(env: Env, admin: Address, payment_token: Address) {
        admin.require_auth();
        if env.storage().persistent().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::PaymentToken, &payment_token);
        env.storage().persistent().set(&DataKey::NextId, &1u64);
    }

    // ──────────────── ADMIN ────────────────

    /// Set (or update) the configuration for a tier.  Admin only.
    pub fn set_tier_config(
        env: Env,
        admin: Address,
        tier: Tier,
        price: i128,
        duration_days: u64,
        puzzle_access_level: u32,
        feature_flags: Vec<String>,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if price < 0 {
            panic!("price must be non-negative");
        }
        if duration_days == 0 {
            panic!("duration_days must be > 0");
        }

        let cfg = TierConfig {
            tier,
            price,
            duration_days,
            puzzle_access_level,
            feature_flags,
        };
        env.storage().persistent().set(&DataKey::TierConfig(tier), &cfg);
    }

    // ──────────────── SUBSCRIBE ────────────────

    /// Subscribe to a tier (or upgrade inline).  Player pays the tier price.
    /// If the player has an existing active subscription it must be expired first;
    /// use `upgrade` for mid-period tier changes.
    pub fn subscribe(env: Env, player: Address, tier: Tier) -> u64 {
        player.require_auth();

        let cfg = Self::get_tier_config_or_panic(&env, tier);

        // If a previous subscription exists and is still active, reject.
        if let Some(sub_id) = Self::player_sub_id(&env, &player) {
            let sub: Subscription = env
                .storage()
                .persistent()
                .get(&DataKey::Subscription(sub_id))
                .unwrap();
            let now = env.ledger().timestamp();
            if sub.expires_at > now {
                panic!("existing subscription still active; use upgrade or wait for expiry");
            }
        }

        // Charge only if tier has a price (Free tier = 0).
        if cfg.price > 0 {
            let payment_token: Address = env
                .storage()
                .persistent()
                .get(&DataKey::PaymentToken)
                .unwrap();
            let token_client = token::Client::new(&env, &payment_token);
            token_client.transfer(&player, &env.current_contract_address(), &cfg.price);
        }

        let now = env.ledger().timestamp();
        let sub_id = Self::next_id(&env);

        let sub = Subscription {
            holder: player.clone(),
            tier,
            started_at: now,
            expires_at: now + cfg.duration_days * SECONDS_PER_DAY,
            auto_renew: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Subscription(sub_id), &sub);
        env.storage()
            .persistent()
            .set(&DataKey::PlayerSub(player.clone()), &sub_id);

        env.events().publish(
            (Symbol::new(&env, EVT_SUBSCRIBED), player),
            (sub_id, tier as u32),
        );

        sub_id
    }

    // ──────────────── RENEW ────────────────

    /// Extend an existing subscription by one period (duration_days).
    /// If `auto_renew` is true anyone may call this; otherwise the holder must sign.
    pub fn renew(env: Env, caller: Address, subscription_id: u64) {
        let mut sub: Subscription = env
            .storage()
            .persistent()
            .get(&DataKey::Subscription(subscription_id))
            .expect("subscription not found");

        if sub.auto_renew {
            // Anyone may trigger auto-renewal; no specific auth required beyond tx signing.
            caller.require_auth();
        } else {
            // Manual renewal requires the holder to authorise.
            sub.holder.require_auth();
        }

        let cfg = Self::get_tier_config_or_panic(&env, sub.tier);

        if cfg.price > 0 {
            let payment_token: Address = env
                .storage()
                .persistent()
                .get(&DataKey::PaymentToken)
                .unwrap();
            let token_client = token::Client::new(&env, &payment_token);
            token_client.transfer(&sub.holder, &env.current_contract_address(), &cfg.price);
        }

        let now = env.ledger().timestamp();
        // Always extend from max(now, current expires_at) so renewals stack properly.
        let base = if sub.expires_at > now {
            sub.expires_at
        } else {
            now
        };
        sub.expires_at = base + cfg.duration_days * SECONDS_PER_DAY;

        env.storage()
            .persistent()
            .set(&DataKey::Subscription(subscription_id), &sub);

        env.events().publish(
            (Symbol::new(&env, EVT_RENEWED), sub.holder),
            (subscription_id, sub.expires_at),
        );
    }

    // ──────────────── UPGRADE ────────────────

    /// Upgrade an active subscription to a higher tier.
    /// Prorates the remaining time: the unused value from the current tier is
    /// subtracted from the new tier's price before charging.
    /// The subscription period is reset to a full period of the new tier.
    pub fn upgrade(env: Env, subscription_id: u64, new_tier: Tier) {
        let mut sub: Subscription = env
            .storage()
            .persistent()
            .get(&DataKey::Subscription(subscription_id))
            .expect("subscription not found");

        sub.holder.require_auth();

        let now = env.ledger().timestamp();
        if sub.expires_at <= now {
            panic!("subscription has expired; please subscribe again");
        }

        if (new_tier as u32) <= (sub.tier as u32) {
            panic!("can only upgrade to a higher tier");
        }

        let old_cfg = Self::get_tier_config_or_panic(&env, sub.tier);
        let new_cfg = Self::get_tier_config_or_panic(&env, new_tier);

        // Proration: remaining fraction of the old period × old price.
        let old_period = old_cfg.duration_days * SECONDS_PER_DAY;
        let remaining_secs = sub.expires_at.saturating_sub(now);

        // remaining_value = old_price * remaining_secs / old_period  (integer arithmetic)
        let remaining_value: i128 = if old_period > 0 && old_cfg.price > 0 {
            (old_cfg.price * remaining_secs as i128) / old_period as i128
        } else {
            0
        };

        let charge = (new_cfg.price - remaining_value).max(0);

        if charge > 0 {
            let payment_token: Address = env
                .storage()
                .persistent()
                .get(&DataKey::PaymentToken)
                .unwrap();
            let token_client = token::Client::new(&env, &payment_token);
            token_client.transfer(&sub.holder, &env.current_contract_address(), &charge);
        }

        let old_tier = sub.tier;
        sub.tier = new_tier;
        sub.expires_at = now + new_cfg.duration_days * SECONDS_PER_DAY;

        env.storage()
            .persistent()
            .set(&DataKey::Subscription(subscription_id), &sub);

        env.events().publish(
            (Symbol::new(&env, EVT_UPGRADED), sub.holder),
            (subscription_id, old_tier as u32, new_tier as u32, charge),
        );
    }

    // ──────────────── CANCEL ────────────────

    /// Cancel a subscription: disables auto-renewal so it runs out at `expires_at`.
    pub fn cancel(env: Env, subscription_id: u64) {
        let mut sub: Subscription = env
            .storage()
            .persistent()
            .get(&DataKey::Subscription(subscription_id))
            .expect("subscription not found");

        sub.holder.require_auth();

        sub.auto_renew = false;

        env.storage()
            .persistent()
            .set(&DataKey::Subscription(subscription_id), &sub);

        env.events().publish(
            (Symbol::new(&env, EVT_CANCELLED), sub.holder),
            subscription_id,
        );
    }

    // ──────────────── ACCESS GATE ────────────────

    /// Returns true if `player` has an active subscription whose tier is
    /// greater than or equal to `required_tier`.
    /// Safe to call from other contracts as a trustless access check.
    pub fn has_access(env: Env, player: Address, required_tier: Tier) -> bool {
        let sub_id = match Self::player_sub_id(&env, &player) {
            Some(id) => id,
            None => return required_tier == Tier::Free,
        };

        let sub: Subscription = match env
            .storage()
            .persistent()
            .get(&DataKey::Subscription(sub_id))
        {
            Some(s) => s,
            None => return required_tier == Tier::Free,
        };

        let now = env.ledger().timestamp();
        if sub.expires_at <= now {
            // Expired subscription — only Free tier passes.
            return required_tier == Tier::Free;
        }

        (sub.tier as u32) >= (required_tier as u32)
    }

    // ──────────────── QUERIES ────────────────

    /// Return the subscription record for the given id.
    pub fn get_subscription(env: Env, subscription_id: u64) -> Subscription {
        env.storage()
            .persistent()
            .get(&DataKey::Subscription(subscription_id))
            .expect("subscription not found")
    }

    /// Return the subscription id for a player, if any.
    pub fn get_player_subscription_id(env: Env, player: Address) -> Option<u64> {
        Self::player_sub_id(&env, &player)
    }

    /// Return the TierConfig for a tier.
    pub fn get_tier_config(env: Env, tier: Tier) -> TierConfig {
        Self::get_tier_config_or_panic(&env, tier)
    }

    /// Enable or disable auto-renew for a subscription.
    pub fn set_auto_renew(env: Env, subscription_id: u64, auto_renew: bool) {
        let mut sub: Subscription = env
            .storage()
            .persistent()
            .get(&DataKey::Subscription(subscription_id))
            .expect("subscription not found");

        sub.holder.require_auth();
        sub.auto_renew = auto_renew;
        env.storage()
            .persistent()
            .set(&DataKey::Subscription(subscription_id), &sub);
    }

    // ──────────────── INTERNAL HELPERS ────────────────

    fn assert_admin(env: &Env, caller: &Address) {
        let admin: Address = env.storage().persistent().get(&DataKey::Admin).unwrap();
        if *caller != admin {
            panic!("caller is not admin");
        }
    }

    fn get_tier_config_or_panic(env: &Env, tier: Tier) -> TierConfig {
        env.storage()
            .persistent()
            .get(&DataKey::TierConfig(tier))
            .expect("tier not configured")
    }

    fn player_sub_id(env: &Env, player: &Address) -> Option<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::PlayerSub(player.clone()))
    }

    fn next_id(env: &Env) -> u64 {
        let id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::NextId)
            .unwrap_or(1);
        env.storage().persistent().set(&DataKey::NextId, &(id + 1));
        id
    }
}
