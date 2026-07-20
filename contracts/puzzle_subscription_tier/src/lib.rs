#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, String, Symbol, Vec};

// ─────────────────────────────────────────────────────────
// TIME CONSTANTS
// ─────────────────────────────────────────────────────────

#[cfg(not(test))]
const SECONDS_PER_DAY: u64 = 86_400;
#[cfg(test)]
const SECONDS_PER_DAY: u64 = 1;

// Maximum number of price-history records kept per tier.
// Oldest entries are dropped once this cap is reached so ledger storage stays bounded.
const MAX_PRICE_HISTORY: u32 = 50;

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
    /// Contract-wide admin address.
    Admin,
    /// Payment token address.
    PaymentToken,
    /// TierConfig keyed by Tier enum value (current / live config).
    TierConfig(Tier),
    /// Subscription record keyed by subscription id.
    Subscription(u64),
    /// Maps player Address → their current subscription id.
    PlayerSub(Address),
    /// Monotonic counter for subscription ids.
    NextId,
    /// Append-only price-history log per tier (Vec<PriceChangeRecord>).
    PriceHistory(Tier),
}

// ─────────────────────────────────────────────────────────
// STRUCTS
// ─────────────────────────────────────────────────────────

/// Per-subscription state stored on-chain.
///
/// `locked_price` and `locked_duration_days` capture the pricing terms
/// that were active when the subscription was **first created or last
/// manually renewed**.  This means existing subscribers are grandfathered:
/// even if the admin later calls `update_tier_price`, their next renewal
/// continues at the price they originally agreed to.
///
/// Only a brand-new subscription (or a re-subscribe after expiry) picks
/// up the then-current `TierConfig` price.
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
    /// Price locked at subscribe time; used for renewals (grandfathering).
    pub locked_price: i128,
    /// Duration locked at subscribe time; used for renewals.
    pub locked_duration_days: u64,
}

/// Configuration for a single tier, set by the admin.
#[contracttype]
#[derive(Clone, Debug)]
pub struct TierConfig {
    /// The tier this config applies to.
    pub tier: Tier,
    /// Token amount required to subscribe (or pay difference on upgrade).
    /// This is the **current** price for brand-new subscriptions.
    pub price: i128,
    /// How many days each subscription period lasts.
    pub duration_days: u64,
    /// Numeric access level exposed to other contracts.
    pub puzzle_access_level: u32,
    /// Arbitrary feature flag strings (e.g. "hints", "leaderboard").
    pub feature_flags: Vec<String>,
}

/// One record in the auditable price-change history for a tier.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PriceChangeRecord {
    /// The tier whose price changed.
    pub tier: Tier,
    /// Price before the change.
    pub old_price: i128,
    /// Price after the change.
    pub new_price: i128,
    /// Duration before the change (for full audit completeness).
    pub old_duration_days: u64,
    /// Duration after the change.
    pub new_duration_days: u64,
    /// Address of the admin who made the change.
    pub changed_by: Address,
    /// Ledger timestamp at which the change was recorded.
    pub changed_at: u64,
}

// ─────────────────────────────────────────────────────────
// EVENTS  (topic symbols)
// ─────────────────────────────────────────────────────────

const EVT_SUBSCRIBED: &str = "subscribed";
const EVT_RENEWED: &str = "renewed";
const EVT_UPGRADED: &str = "upgraded";
const EVT_CANCELLED: &str = "cancelled";
const EVT_PRICE_UPDATED: &str = "price_updated";

// ─────────────────────────────────────────────────────────
// CONTRACT
// ─────────────────────────────────────────────────────────

#[contract]
pub struct PuzzleSubscriptionTierContract;

#[contractimpl]
impl PuzzleSubscriptionTierContract {
    // ──────────────── INITIALIZATION ────────────────

    /// Initialize the contract. Must be called once before any other function.
    pub fn initialize(env: Env, admin: Address, payment_token: Address) {
        admin.require_auth();
        if env.storage().persistent().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::PaymentToken, &payment_token);
        env.storage().persistent().set(&DataKey::NextId, &1u64);
    }

    // ──────────────── ADMIN — TIER CONFIG ────────────────

    /// Set (or update) the full configuration for a tier. Admin only.
    ///
    /// This function does **not** touch the `PriceHistory` log.
    /// To change only the price and have it recorded in the audit trail,
    /// use `update_tier_price` instead.
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
        env.storage()
            .persistent()
            .set(&DataKey::TierConfig(tier), &cfg);
    }

    // ──────────────── ADMIN — DYNAMIC PRICING ────────────────

    /// Update the price (and optionally the duration) for an existing tier.
    /// Admin only.
    ///
    /// ## Semantics
    /// - The new price takes effect for **new subscriptions** immediately
    ///   after this call.
    /// - **Active subscribers** are grandfathered: their `locked_price` and
    ///   `locked_duration_days` are unchanged, so their next renewal charges
    ///   what they originally agreed to pay.
    /// - A `PriceChangeRecord` is appended to the on-chain audit log for this
    ///   tier and a `price_updated` event is emitted so off-chain indexers
    ///   can track every change.
    ///
    /// # Panics
    /// - `"caller is not admin"` — if caller is not the stored admin.
    /// - `"tier not configured"` — if the tier has no existing config
    ///   (call `set_tier_config` first).
    /// - `"new_price must be non-negative"` — on negative price.
    /// - `"new_duration_days must be > 0"` — on zero duration.
    pub fn update_tier_price(
        env: Env,
        admin: Address,
        tier: Tier,
        new_price: i128,
        new_duration_days: u64,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if new_price < 0 {
            panic!("new_price must be non-negative");
        }
        if new_duration_days == 0 {
            panic!("new_duration_days must be > 0");
        }

        let mut cfg = Self::get_tier_config_or_panic(&env, tier);

        let old_price = cfg.price;
        let old_duration_days = cfg.duration_days;

        // Record change in audit log before updating the config.
        let record = PriceChangeRecord {
            tier,
            old_price,
            new_price,
            old_duration_days,
            new_duration_days,
            changed_by: admin.clone(),
            changed_at: env.ledger().timestamp(),
        };
        Self::append_price_history(&env, tier, record);

        // Update the live TierConfig.
        cfg.price = new_price;
        cfg.duration_days = new_duration_days;
        env.storage()
            .persistent()
            .set(&DataKey::TierConfig(tier), &cfg);

        // Emit auditable event.
        env.events().publish(
            (Symbol::new(&env, EVT_PRICE_UPDATED), admin),
            (tier as u32, old_price, new_price, old_duration_days, new_duration_days),
        );
    }

    // ──────────────── SUBSCRIBE ────────────────

    /// Subscribe to a tier (or upgrade inline). Player pays the **current**
    /// tier price at the time of this call.
    ///
    /// If the player has an existing active subscription it must be expired first;
    /// use `upgrade` for mid-period tier changes.
    ///
    /// The price and duration are **locked into the Subscription record**
    /// so future renewals honour the grandfathered rate even if the admin
    /// later changes the price via `update_tier_price`.
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
            // Lock the price and duration active at subscribe time.
            locked_price: cfg.price,
            locked_duration_days: cfg.duration_days,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Subscription(sub_id), &sub);
        env.storage()
            .persistent()
            .set(&DataKey::PlayerSub(player.clone()), &sub_id);

        env.events().publish(
            (Symbol::new(&env, EVT_SUBSCRIBED), player),
            (sub_id, tier as u32, cfg.price),
        );

        sub_id
    }

    // ──────────────── RENEW ────────────────

    /// Extend an existing subscription by one period.
    ///
    /// ## Grandfathering
    /// The renewal charge and duration come from the subscription's
    /// **`locked_price` / `locked_duration_days`**, not from the current
    /// `TierConfig`. This means price changes made by the admin do NOT affect
    /// subscribers who are already paying an older rate — they keep renewing
    /// at the price they originally agreed to.
    ///
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

        // Use the locked price (grandfathered rate) rather than the current TierConfig price.
        let price = sub.locked_price;
        let duration_days = sub.locked_duration_days;

        if price > 0 {
            let payment_token: Address = env
                .storage()
                .persistent()
                .get(&DataKey::PaymentToken)
                .unwrap();
            let token_client = token::Client::new(&env, &payment_token);
            token_client.transfer(&sub.holder, &env.current_contract_address(), &price);
        }

        let now = env.ledger().timestamp();
        // Always extend from max(now, current expires_at) so renewals stack properly.
        let base = if sub.expires_at > now {
            sub.expires_at
        } else {
            now
        };
        sub.expires_at = base + duration_days * SECONDS_PER_DAY;

        env.storage()
            .persistent()
            .set(&DataKey::Subscription(subscription_id), &sub);

        env.events().publish(
            (Symbol::new(&env, EVT_RENEWED), sub.holder),
            (subscription_id, sub.expires_at, price),
        );
    }

    // ──────────────── UPGRADE ────────────────

    /// Upgrade an active subscription to a higher tier.
    ///
    /// ## Pricing
    /// The upgrade charge is computed using:
    /// - **Old side**: the subscription's `locked_price` and `locked_duration_days`
    ///   (the grandfathered terms).
    /// - **New side**: the **current** `TierConfig` price and duration for the
    ///   target tier (the scout explicitly chooses to move to a new tier, so
    ///   they see the live market price).
    ///
    /// Proration formula:
    /// ```
    /// remaining_value = locked_price * remaining_secs / locked_period_secs
    /// charge = max(0, new_cfg.price - remaining_value)
    /// ```
    ///
    /// The new locked price/duration are set from the target tier's current config.
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

        // Old-side proration uses the locked (grandfathered) terms.
        let old_locked_period = sub.locked_duration_days * SECONDS_PER_DAY;
        let remaining_secs = sub.expires_at.saturating_sub(now);

        let remaining_value: i128 = if old_locked_period > 0 && sub.locked_price > 0 {
            (sub.locked_price * remaining_secs as i128) / old_locked_period as i128
        } else {
            0
        };

        // New-side uses the current (live) TierConfig.
        let new_cfg = Self::get_tier_config_or_panic(&env, new_tier);

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

        // Lock in the new tier's current price/duration so subsequent renewals
        // also see the price the subscriber agreed to at upgrade time.
        sub.locked_price = new_cfg.price;
        sub.locked_duration_days = new_cfg.duration_days;

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

    /// Return the full price-change history for a tier, oldest-first.
    ///
    /// Each entry records: old price, new price, old/new duration, who changed it,
    /// and when.  At most `MAX_PRICE_HISTORY` entries are stored; older entries
    /// are dropped automatically to keep ledger storage bounded.
    pub fn get_price_history(env: Env, tier: Tier) -> Vec<PriceChangeRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::PriceHistory(tier))
            .unwrap_or_else(|| Vec::new(&env))
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

    /// Append a `PriceChangeRecord` to the bounded history log for `tier`.
    /// If the log is at capacity, the oldest entry is dropped.
    fn append_price_history(env: &Env, tier: Tier, record: PriceChangeRecord) {
        let key = DataKey::PriceHistory(tier);
        let mut history: Vec<PriceChangeRecord> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));

        // If at capacity, drop the oldest entry (index 0) to stay bounded.
        if history.len() >= MAX_PRICE_HISTORY {
            // Build a new vec skipping the first element.
            let mut trimmed: Vec<PriceChangeRecord> = Vec::new(env);
            for i in 1..history.len() {
                trimmed.push_back(history.get(i).unwrap());
            }
            history = trimmed;
        }

        history.push_back(record);
        env.storage().persistent().set(&key, &history);
    }
}

#[cfg(test)]
mod tests;
