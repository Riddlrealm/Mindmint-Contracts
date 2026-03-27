#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, token, Address, Env, Symbol, Vec};

const MAX_BPS: u32 = 10_000;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Tracks cumulative burned amounts by burn type.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BurnStats {
    pub total_burned_voluntary: i128,
    pub total_burned_fee: i128,
    pub total_burned_event: i128,
}

impl BurnStats {
    pub fn total(&self) -> i128 {
        self.total_burned_voluntary + self.total_burned_fee + self.total_burned_event
    }
}

/// Single entry in the immutable burn history log.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BurnRecord {
    /// Amount of tokens burned.
    pub amount: i128,
    /// Address whose tokens were burned.
    pub source: Address,
    /// `voluntary`, `fee`, or a custom event symbol.
    pub reason: Symbol,
    /// Ledger timestamp at burn time.
    pub timestamp: u64,
}

/// Contract-wide configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BurnConfig {
    pub admin: Address,
    /// Token contract whose `burn` function is invoked.
    pub token: Address,
    /// Fee burn rate in basis points (100 bps = 1 %).
    pub burn_rate_bps: u32,
    /// When false, fee_burn and event_burn are no-ops; voluntary_burn still works.
    pub enabled: bool,
}

#[contracttype]
pub enum DataKey {
    Config,
    Stats,
    HistoryCount,
    History(u64),
    Distributor(Address),
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct TokenBurn;

#[contractimpl]
impl TokenBurn {
    // -----------------------------------------------------------------------
    // Initialisation
    // -----------------------------------------------------------------------

    pub fn initialize(env: Env, admin: Address, token: Address, burn_rate_bps: u32) {
        if env.storage().instance().has(&DataKey::Config) {
            panic!("already_initialized");
        }
        if burn_rate_bps > MAX_BPS {
            panic!("burn_rate_exceeds_100_pct");
        }

        let config = BurnConfig { admin, token, burn_rate_bps, enabled: true };
        env.storage().instance().set(&DataKey::Config, &config);
        env.storage().instance().set(&DataKey::Stats, &BurnStats {
            total_burned_voluntary: 0,
            total_burned_fee: 0,
            total_burned_event: 0,
        });
        env.storage().instance().set(&DataKey::HistoryCount, &0u64);
    }

    // -----------------------------------------------------------------------
    // Admin controls
    // -----------------------------------------------------------------------

    /// Update the fee burn rate (admin only).
    pub fn set_burn_rate(env: Env, burn_rate_bps: u32) {
        let mut config = Self::load_config(&env);
        config.admin.require_auth();
        if burn_rate_bps > MAX_BPS {
            panic!("burn_rate_exceeds_100_pct");
        }
        config.burn_rate_bps = burn_rate_bps;
        env.storage().instance().set(&DataKey::Config, &config);
    }

    /// Enable or disable automatic burns (admin only).
    pub fn set_enabled(env: Env, enabled: bool) {
        let mut config = Self::load_config(&env);
        config.admin.require_auth();
        config.enabled = enabled;
        env.storage().instance().set(&DataKey::Config, &config);
    }

    /// Grant an address the right to trigger event burns (admin only).
    pub fn authorize_distributor(env: Env, distributor: Address) {
        let config = Self::load_config(&env);
        config.admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Distributor(distributor), &true);
    }

    /// Revoke event-burn rights from an address (admin only).
    pub fn revoke_distributor(env: Env, distributor: Address) {
        let config = Self::load_config(&env);
        config.admin.require_auth();
        env.storage()
            .persistent()
            .remove(&DataKey::Distributor(distributor));
    }

    // -----------------------------------------------------------------------
    // Burn mechanisms
    // -----------------------------------------------------------------------

    /// Voluntary burn: caller permanently destroys `amount` of their own tokens.
    /// The caller must have authorized this contract to act on their behalf.
    pub fn voluntary_burn(env: Env, caller: Address, amount: i128) -> i128 {
        caller.require_auth();
        if amount <= 0 {
            panic!("invalid_amount");
        }

        let config = Self::load_config(&env);
        token::Client::new(&env, &config.token).burn(&caller, &amount);

        Self::record(&env, amount, &caller, symbol_short!("voluntary"), false, false);

        env.events().publish((symbol_short!("burn_vol"), caller), amount);
        amount
    }

    /// Transaction fee burn: deducts `burn_rate_bps` % of `tx_amount` from
    /// `caller` and burns it.  Returns the fee burned (0 if burns are disabled).
    pub fn fee_burn(env: Env, caller: Address, tx_amount: i128) -> i128 {
        caller.require_auth();
        if tx_amount <= 0 {
            panic!("invalid_amount");
        }

        let config = Self::load_config(&env);
        if !config.enabled {
            return 0;
        }

        let fee = tx_amount
            .checked_mul(config.burn_rate_bps as i128)
            .unwrap_or_else(|| panic!("overflow"))
            / MAX_BPS as i128;

        if fee <= 0 {
            return 0;
        }

        token::Client::new(&env, &config.token).burn(&caller, &fee);
        Self::record(&env, fee, &caller, symbol_short!("fee"), true, false);

        env.events().publish((symbol_short!("burn_fee"), caller), fee);
        fee
    }

    /// Event-triggered burn: an authorized distributor burns `amount` from their
    /// own balance for a named in-game event (e.g. seasonal deflation, raid reward).
    pub fn event_burn(env: Env, distributor: Address, amount: i128, event_name: Symbol) {
        distributor.require_auth();
        if amount <= 0 {
            panic!("invalid_amount");
        }

        let config = Self::load_config(&env);
        if !config.enabled {
            panic!("burns_disabled");
        }
        if !env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::Distributor(distributor.clone()))
            .unwrap_or(false)
        {
            panic!("not_authorized_distributor");
        }

        token::Client::new(&env, &config.token).burn(&distributor, &amount);
        Self::record(&env, amount, &distributor, event_name.clone(), false, true);

        env.events().publish((symbol_short!("burn_evt"), distributor), (event_name, amount));
    }

    // -----------------------------------------------------------------------
    // Statistics & history queries
    // -----------------------------------------------------------------------

    /// Returns cumulative burn totals broken down by burn type.
    pub fn get_stats(env: Env) -> BurnStats {
        env.storage().instance().get(&DataKey::Stats).unwrap_or(BurnStats {
            total_burned_voluntary: 0,
            total_burned_fee: 0,
            total_burned_event: 0,
        })
    }

    /// Returns the combined total of all tokens burned by this contract.
    pub fn get_total_burned(env: Env) -> i128 {
        Self::get_stats(env).total()
    }

    /// Returns the current configuration.
    pub fn get_config(env: Env) -> BurnConfig {
        Self::load_config(&env)
    }

    /// Returns a paginated slice of the burn history log.
    pub fn get_history(env: Env, offset: u64, limit: u64) -> Vec<BurnRecord> {
        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::HistoryCount)
            .unwrap_or(0);
        let mut out = Vec::new(&env);

        if offset >= count {
            return out;
        }

        let end = (offset + limit).min(count);
        for i in offset..end {
            if let Some(record) = env
                .storage()
                .persistent()
                .get::<DataKey, BurnRecord>(&DataKey::History(i))
            {
                out.push_back(record);
            }
        }
        out
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn load_config(env: &Env) -> BurnConfig {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .unwrap_or_else(|| panic!("not_initialized"))
    }

    /// Update stats and append a history entry.
    fn record(env: &Env, amount: i128, source: &Address, reason: Symbol, is_fee: bool, is_event: bool) {
        // Update cumulative stats.
        let mut stats: BurnStats = env
            .storage()
            .instance()
            .get(&DataKey::Stats)
            .unwrap_or(BurnStats {
                total_burned_voluntary: 0,
                total_burned_fee: 0,
                total_burned_event: 0,
            });

        if is_fee {
            stats.total_burned_fee += amount;
        } else if is_event {
            stats.total_burned_event += amount;
        } else {
            stats.total_burned_voluntary += amount;
        }
        env.storage().instance().set(&DataKey::Stats, &stats);

        // Append history record (persistent storage for durability).
        let idx: u64 = env
            .storage()
            .instance()
            .get(&DataKey::HistoryCount)
            .unwrap_or(0);

        let record = BurnRecord {
            amount,
            source: source.clone(),
            reason,
            timestamp: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::History(idx), &record);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::History(idx), 100_000, 500_000);
        env.storage()
            .instance()
            .set(&DataKey::HistoryCount, &(idx + 1));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, token};

    fn setup(env: &Env, burn_rate_bps: u32) -> (Address, token::Client, token::StellarAssetClient, Address, TokenBurnClient) {
        let admin = Address::generate(env);
        let token_addr = env.register_stellar_asset_contract_v2(admin.clone()).address();
        let token_client = token::Client::new(env, &token_addr);
        let token_admin = token::StellarAssetClient::new(env, &token_addr);

        let contract_id = env.register_contract(None, TokenBurn);
        let client = TokenBurnClient::new(env, &contract_id);
        client.initialize(&admin, &token_addr, &burn_rate_bps);

        (admin, token_client, token_admin, token_addr, client)
    }

    #[test]
    fn test_voluntary_burn_reduces_supply() {
        let env = Env::default();
        env.mock_all_auths();

        let (admin, token, token_admin, _, client) = setup(&env, 100);
        let user = Address::generate(&env);
        token_admin.mint(&user, &5_000);

        client.voluntary_burn(&user, &2_000);

        assert_eq!(token.balance(&user), 3_000);
        let stats = client.get_stats();
        assert_eq!(stats.total_burned_voluntary, 2_000);
        assert_eq!(client.get_total_burned(), 2_000);
    }

    #[test]
    fn test_fee_burn_correct_amount() {
        let env = Env::default();
        env.mock_all_auths();

        // 200 bps = 2 %
        let (_, token, token_admin, _, client) = setup(&env, 200);
        let user = Address::generate(&env);
        token_admin.mint(&user, &10_000);

        // tx_amount = 5_000  ⟹  fee = 5_000 * 200 / 10_000 = 100
        let fee = client.fee_burn(&user, &5_000);

        assert_eq!(fee, 100);
        assert_eq!(token.balance(&user), 9_900);
        let stats = client.get_stats();
        assert_eq!(stats.total_burned_fee, 100);
    }

    #[test]
    fn test_fee_burn_disabled_returns_zero() {
        let env = Env::default();
        env.mock_all_auths();

        let (_, token, token_admin, _, client) = setup(&env, 200);
        let user = Address::generate(&env);
        token_admin.mint(&user, &10_000);

        client.set_enabled(&false);
        let fee = client.fee_burn(&user, &5_000);

        assert_eq!(fee, 0);
        assert_eq!(token.balance(&user), 10_000); // no tokens burned
    }

    #[test]
    fn test_event_burn_by_authorized_distributor() {
        let env = Env::default();
        env.mock_all_auths();

        let (admin, token, token_admin, _, client) = setup(&env, 0);
        let distributor = Address::generate(&env);
        token_admin.mint(&distributor, &10_000);

        client.authorize_distributor(&distributor);
        client.event_burn(&distributor, &3_000, &symbol_short!("raid"));

        assert_eq!(token.balance(&distributor), 7_000);
        let stats = client.get_stats();
        assert_eq!(stats.total_burned_event, 3_000);
    }

    #[test]
    #[should_panic(expected = "not_authorized_distributor")]
    fn test_event_burn_unauthorized() {
        let env = Env::default();
        env.mock_all_auths();

        let (_, _, token_admin, _, client) = setup(&env, 0);
        let stranger = Address::generate(&env);
        token_admin.mint(&stranger, &5_000);

        client.event_burn(&stranger, &1_000, &symbol_short!("raid"));
    }

    #[test]
    fn test_history_pagination() {
        let env = Env::default();
        env.mock_all_auths();

        let (_, _, token_admin, _, client) = setup(&env, 0);
        let user = Address::generate(&env);
        token_admin.mint(&user, &100_000);

        client.voluntary_burn(&user, &1_000);
        client.voluntary_burn(&user, &2_000);
        client.voluntary_burn(&user, &3_000);

        let page = client.get_history(&1, &2);
        assert_eq!(page.len(), 2);
        assert_eq!(page.get(0).unwrap().amount, 2_000);
        assert_eq!(page.get(1).unwrap().amount, 3_000);
    }

    #[test]
    fn test_cumulative_stats_all_types() {
        let env = Env::default();
        env.mock_all_auths();

        let (_, _, token_admin, _, client) = setup(&env, 500); // 5 %
        let user = Address::generate(&env);
        let distributor = Address::generate(&env);
        token_admin.mint(&user, &100_000);
        token_admin.mint(&distributor, &50_000);

        client.authorize_distributor(&distributor);

        client.voluntary_burn(&user, &1_000);             // voluntary: 1_000
        client.fee_burn(&user, &10_000);                   // fee: 10_000*500/10_000 = 500
        client.event_burn(&distributor, &2_000, &symbol_short!("season")); // event: 2_000

        let stats = client.get_stats();
        assert_eq!(stats.total_burned_voluntary, 1_000);
        assert_eq!(stats.total_burned_fee, 500);
        assert_eq!(stats.total_burned_event, 2_000);
        assert_eq!(client.get_total_burned(), 3_500);
    }

    #[test]
    #[should_panic(expected = "burn_rate_exceeds_100_pct")]
    fn test_invalid_burn_rate() {
        let env = Env::default();
        env.mock_all_auths();

        let (_, _, _, _, client) = setup(&env, 100);
        client.set_burn_rate(&10_001);
    }
}
