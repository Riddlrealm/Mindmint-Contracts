#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    token, Address, Env, Map, String, Vec,
};

// ──────────────────────────────────────────────────────────
// CONSTANTS
// ──────────────────────────────────────────────────────────

/// Maximum upgrade tier allowed. Attempts to target a tier above this are rejected.
const MAX_TIER: u32 = 5;

/// Failed upgrades refund cost / REFUND_DIVISOR tokens back to the player (50 %).
const REFUND_DIVISOR: i128 = 2;

/// Basis-points denominator: 10_000 = 100 %.
const BPS_DENOMINATOR: u64 = 10_000;

// ──────────────────────────────────────────────────────────
// DATA KEYS
// ──────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Config,
    UpgradeConfig(u32),
    UpgradeHistory(u32),
    NftAttributes(u32),
}

// ──────────────────────────────────────────────────────────
// STRUCTS
// ──────────────────────────────────────────────────────────

/// Contract-level configuration set at initialisation.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ContractConfig {
    pub admin: Address,
    pub token: Address,
    pub nft_contract: Address,
}

/// Per-tier upgrade configuration registered by admin.
#[contracttype]
#[derive(Clone, Debug)]
pub struct UpgradeConfig {
    /// Tier number (1 – MAX_TIER).
    pub tier: u32,
    /// Token cost (in the smallest unit) deducted upfront.
    pub cost: i128,
    /// Success probability in basis points (0 – 10_000).
    pub success_rate_bps: u32,
    /// Attribute name → boost value applied on a successful upgrade.
    pub attribute_boosts: Map<String, u32>,
}

/// Immutable record written after every upgrade attempt.
#[contracttype]
#[derive(Clone, Debug)]
pub struct UpgradeAttempt {
    pub nft_id: u32,
    pub player: Address,
    pub tokens_spent: i128,
    pub success: bool,
    /// Attributes as they stand after this attempt (unchanged on failure).
    pub new_attributes: Map<String, u32>,
    pub attempted_at: u64,
}

// ──────────────────────────────────────────────────────────
// CONTRACT
// ──────────────────────────────────────────────────────────

#[contract]
pub struct NftUpgradeContract;

#[contractimpl]
impl NftUpgradeContract {
    // ───────────── INITIALISATION ─────────────

    pub fn initialize(env: Env, admin: Address, token: Address, nft_contract: Address) {
        if env.storage().instance().has(&DataKey::Config) {
            panic!("AlreadyInitialized");
        }
        admin.require_auth();
        env.storage().instance().set(
            &DataKey::Config,
            &ContractConfig { admin, token, nft_contract },
        );
    }

    // ───────────── ADMIN: CONFIG ─────────────

    /// Register (or overwrite) the upgrade configuration for `tier`.
    pub fn register_upgrade_config(
        env: Env,
        tier: u32,
        cost: i128,
        success_rate_bps: u32,
        attribute_boosts: Map<String, u32>,
    ) {
        let config = Self::load_config(&env);
        config.admin.require_auth();

        if tier == 0 || tier > MAX_TIER {
            panic!("MaxTierReached");
        }

        let clamped_bps = if success_rate_bps > BPS_DENOMINATOR as u32 {
            BPS_DENOMINATOR as u32
        } else {
            success_rate_bps
        };

        let key = DataKey::UpgradeConfig(tier);
        env.storage().persistent().set(
            &key,
            &UpgradeConfig { tier, cost, success_rate_bps: clamped_bps, attribute_boosts },
        );
        env.storage().persistent().extend_ttl(&key, 100_000, 500_000);

        env.events()
            .publish((symbol_short!("cfg_set"), tier), (cost, clamped_bps));
    }

    // ───────────── CORE: UPGRADE ATTEMPT ─────────────

    /// Attempt to upgrade NFT `nft_id` to `target_tier`.
    ///
    /// 1. Validates tier config and `target_tier ≤ MAX_TIER`.
    /// 2. Optionally verifies ownership via a cross-contract `owner_of` call
    ///    (skipped gracefully if the NFT contract does not expose that method).
    /// 3. Transfers the full `cost` from `player` to this contract.
    /// 4. Rolls an on-chain pseudo-random number against `success_rate_bps`.
    /// 5a. Success  → applies attribute boosts, persists them, emits UpgradeSucceeded.
    /// 5b. Failure  → refunds 50 % of cost back to player, emits UpgradeFailed.
    /// 6. Appends an `UpgradeAttempt` record to the NFT's upgrade history.
    ///
    /// Returns `true` on success, `false` on failure.
    pub fn attempt_upgrade(env: Env, player: Address, nft_id: u32, target_tier: u32) -> bool {
        player.require_auth();

        let config = Self::load_config(&env);

        if target_tier == 0 || target_tier > MAX_TIER {
            panic!("MaxTierReached");
        }

        let upgrade_config: UpgradeConfig = env
            .storage()
            .persistent()
            .get(&DataKey::UpgradeConfig(target_tier))
            .unwrap_or_else(|| panic!("TierNotFound"));

        // Optional ownership check: if the NFT contract exposes `owner_of`,
        // verify the caller owns the NFT. Failure of the cross-contract call
        // (e.g., function not present) is treated as a pass since the player
        // has already authenticated above.
        let owner_result = env.try_invoke_contract::<Address, soroban_sdk::Error>(
            &config.nft_contract,
            &symbol_short!("owner_of"),
            soroban_sdk::vec![&env, nft_id.into()],
        );
        // only enforce if the call succeeded (Ok(Ok(addr)))
        if let Ok(Ok(owner)) = owner_result {
            if owner != player {
                panic!("NotNftOwner");
            }
        }

        let cost = upgrade_config.cost;

        // Deduct cost upfront
        let token_client = token::Client::new(&env, &config.token);
        token_client.transfer(&player, &env.current_contract_address(), &cost);

        // Pseudo-random success roll
        let roll: u64 = env.prng().gen_range(0u64..BPS_DENOMINATOR);
        let success = roll < upgrade_config.success_rate_bps as u64;

        // Current attributes (empty map for a brand-new NFT)
        let mut attributes: Map<String, u32> = env
            .storage()
            .persistent()
            .get(&DataKey::NftAttributes(nft_id))
            .unwrap_or_else(|| Map::new(&env));

        let timestamp = env.ledger().timestamp();

        if success {
            // Apply attribute boosts
            for (name, boost) in upgrade_config.attribute_boosts.iter() {
                let current: u32 = attributes.get(name.clone()).unwrap_or(0);
                attributes.set(name, current.saturating_add(boost));
            }

            // Persist updated attributes
            let attr_key = DataKey::NftAttributes(nft_id);
            env.storage().persistent().set(&attr_key, &attributes);
            env.storage().persistent().extend_ttl(&attr_key, 100_000, 500_000);

            // Best-effort cross-contract notification to the NFT contract
            let meta = String::from_str(&env, "upgraded");
            let _ = env.try_invoke_contract::<soroban_sdk::Val, soroban_sdk::Error>(
                &config.nft_contract,
                &symbol_short!("upd_attr"),
                soroban_sdk::vec![&env, nft_id.into(), meta.into()],
            );

            env.events().publish(
                (symbol_short!("upg_ok"), nft_id),
                (player.clone(), target_tier, attributes.clone()),
            );

            Self::append_history(&env, nft_id, player, cost, true, attributes, timestamp);
            true
        } else {
            // Refund 50 % of cost
            let refund = cost / REFUND_DIVISOR;
            token_client.transfer(&env.current_contract_address(), &player, &refund);

            env.events().publish(
                (symbol_short!("upg_fail"), nft_id),
                (player.clone(), refund, attributes.clone()),
            );

            Self::append_history(&env, nft_id, player, cost, false, attributes, timestamp);
            false
        }
    }

    // ───────────── QUERIES ─────────────

    /// Returns all upgrade attempts recorded for `nft_id`.
    pub fn get_upgrade_history(env: Env, nft_id: u32) -> Vec<UpgradeAttempt> {
        env.storage()
            .persistent()
            .get(&DataKey::UpgradeHistory(nft_id))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns the upgrade configuration for the given `tier`.
    pub fn get_config(env: Env, tier: u32) -> UpgradeConfig {
        env.storage()
            .persistent()
            .get(&DataKey::UpgradeConfig(tier))
            .unwrap_or_else(|| panic!("TierNotFound"))
    }

    /// Returns the current on-chain attribute map for `nft_id`.
    pub fn get_nft_attributes(env: Env, nft_id: u32) -> Map<String, u32> {
        env.storage()
            .persistent()
            .get(&DataKey::NftAttributes(nft_id))
            .unwrap_or_else(|| Map::new(&env))
    }

    /// Returns the contract-level configuration.
    pub fn get_contract_config_pub(env: Env) -> ContractConfig {
        Self::load_config(&env)
    }

    // ───────────── INTERNALS ─────────────

    fn load_config(env: &Env) -> ContractConfig {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .unwrap_or_else(|| panic!("NotInitialized"))
    }

    fn append_history(
        env: &Env,
        nft_id: u32,
        player: Address,
        tokens_spent: i128,
        success: bool,
        new_attributes: Map<String, u32>,
        attempted_at: u64,
    ) {
        let key = DataKey::UpgradeHistory(nft_id);
        let mut history: Vec<UpgradeAttempt> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));

        history.push_back(UpgradeAttempt {
            nft_id,
            player,
            tokens_spent,
            success,
            new_attributes,
            attempted_at,
        });

        env.storage().persistent().set(&key, &history);
        env.storage().persistent().extend_ttl(&key, 100_000, 500_000);
    }
}

#[cfg(test)]
mod test;
