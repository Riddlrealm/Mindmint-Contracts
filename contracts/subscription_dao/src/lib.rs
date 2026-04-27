#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, token, Address, Env, Vec, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SubscriptionDaoError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    PeriodNotFound = 4,
    PeriodNotClosed = 5,
    AlreadyClaimed = 6,
    NotEligible = 7,
    InvalidAmount = 8,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Initialized,
    GovernanceToken,
    CurrentPeriodId,
    RevenuePool(u64),
    Participation(u64, Address), // period_id, player
    Claim(u64, Address),         // period_id, player
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct RevenuePool {
    pub period_id: u64,
    pub balance: i128,
    pub eligible_participants: Vec<Address>,
    pub distributed: bool,
    pub snapshot_at: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct EligibilityInfo {
    pub is_eligible: bool,
    pub share_amount: i128,
}

#[contract]
pub struct SubscriptionDao;

#[contractimpl]
impl SubscriptionDao {
    /// Initialize the contract
    pub fn initialize(env: Env, admin: Address, governance_token: Address) {
        admin.require_auth();

        if env.storage().instance().has(&DataKey::Initialized) {
            panic!("Already initialized");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::GovernanceToken, &governance_token);
        env.storage().instance().set(&DataKey::CurrentPeriodId, &1u64);
        env.storage().instance().set(&DataKey::Initialized, &true);

        // Initialize first period
        let pool = RevenuePool {
            period_id: 1,
            balance: 0,
            eligible_participants: Vec::new(&env),
            distributed: false,
            snapshot_at: 0,
        };
        env.storage().persistent().set(&DataKey::RevenuePool(1), &pool);
    }

    /// Deposit revenue to current period (callable by platform fee oracle)
    /// Note: Tokens should be transferred to the contract before calling this function
    pub fn deposit_revenue(env: Env, amount: i128) {
        Self::require_init(&env);

        if amount <= 0 {
            panic!("Invalid amount");
        }

        let current_period_id = env.storage().instance().get::<_, u64>(&DataKey::CurrentPeriodId).unwrap();
        let mut pool = env.storage().persistent().get::<_, RevenuePool>(&DataKey::RevenuePool(current_period_id)).unwrap();

        if pool.distributed {
            panic!("Period already distributed");
        }

        pool.balance += amount;
        env.storage().persistent().set(&DataKey::RevenuePool(current_period_id), &pool);

        env.events().publish(
            (Symbol::short("rev_dep"), current_period_id),
            amount,
        );
    }

    /// Record player participation for current period (oracle function)
    pub fn record_participation(env: Env, player: Address) {
        Self::require_init(&env);

        let current_period_id = env.storage().instance().get::<_, u64>(&DataKey::CurrentPeriodId).unwrap();
        let mut pool = env.storage().persistent().get::<_, RevenuePool>(&DataKey::RevenuePool(current_period_id)).unwrap();

        if pool.distributed {
            panic!("Period already distributed");
        }

        // Check if already recorded
        if env.storage().persistent().has(&DataKey::Participation(current_period_id, player.clone())) {
            return;
        }

        // Record participation
        env.storage().persistent().set(&DataKey::Participation(current_period_id, player.clone()), &true);
        pool.eligible_participants.push_back(player.clone());
        env.storage().persistent().set(&DataKey::RevenuePool(current_period_id), &pool);

        env.events().publish(
            (Symbol::short("part_rec"), current_period_id),
            player,
        );
    }

    /// Close current period and take snapshot (admin only)
    pub fn close_period(env: Env, admin: Address) {
        admin.require_auth();
        Self::require_admin(&env);

        let current_period_id = env.storage().instance().get::<_, u64>(&DataKey::CurrentPeriodId).unwrap();
        let mut pool = env.storage().persistent().get::<_, RevenuePool>(&DataKey::RevenuePool(current_period_id)).unwrap();

        if pool.distributed {
            panic!("Period already distributed");
        }

        // Take snapshot
        pool.snapshot_at = env.ledger().timestamp();
        pool.distributed = true;
        env.storage().persistent().set(&DataKey::RevenuePool(current_period_id), &pool);

        // Create next period
        let next_period_id = current_period_id + 1;
        let next_pool = RevenuePool {
            period_id: next_period_id,
            balance: 0,
            eligible_participants: Vec::new(&env),
            distributed: false,
            snapshot_at: 0,
        };
        env.storage().persistent().set(&DataKey::RevenuePool(next_period_id), &next_pool);
        env.storage().instance().set(&DataKey::CurrentPeriodId, &next_period_id);

        env.events().publish(
            (Symbol::short("per_cls"), current_period_id),
            pool.snapshot_at,
        );
    }

    /// Claim share for a specific period
    /// Note: Actual token transfer should be handled externally after calling this
    pub fn claim_period_share(env: Env, period_id: u64, player: Address) -> i128 {
        player.require_auth();
        Self::require_init(&env);

        let pool = env.storage().persistent().get::<_, RevenuePool>(&DataKey::RevenuePool(period_id))
            .expect("Period not found");

        if !pool.distributed {
            panic!("Period not closed");
        }

        // Check if already claimed
        if env.storage().persistent().has(&DataKey::Claim(period_id, player.clone())) {
            panic!("Already claimed");
        }

        // Check eligibility: must have participated AND hold governance tokens at snapshot
        let participated = env.storage().persistent().has(&DataKey::Participation(period_id, player.clone()));
        if !participated {
            panic!("Not eligible: did not participate");
        }

        let token_address = env.storage().instance().get::<_, Address>(&DataKey::GovernanceToken).unwrap();
        let token_client = token::Client::new(&env, &token_address);
        let balance = token_client.balance(&player);

        if balance <= 0 {
            panic!("Not eligible: no governance tokens");
        }

        // Calculate share (equal distribution among eligible participants)
        let participant_count = pool.eligible_participants.len();
        if participant_count == 0 {
            panic!("No eligible participants");
        }

        let share = pool.balance / participant_count as i128;

        // Mark as claimed
        env.storage().persistent().set(&DataKey::Claim(period_id, player.clone()), &true);

        env.events().publish(
            (Symbol::short("shr_claim"), period_id, player),
            share,
        );

        share
    }

    /// Get period information
    pub fn get_period(env: Env, period_id: u64) -> RevenuePool {
        env.storage().persistent().get::<_, RevenuePool>(&DataKey::RevenuePool(period_id))
            .expect("Period not found")
    }

    /// Get player eligibility for a period
    pub fn get_player_eligibility(env: Env, period_id: u64, player: Address) -> EligibilityInfo {
        let pool = env.storage().persistent().get::<_, RevenuePool>(&DataKey::RevenuePool(period_id))
            .expect("Period not found");

        let participated = env.storage().persistent().has(&DataKey::Participation(period_id, player.clone()));
        let already_claimed = env.storage().persistent().has(&DataKey::Claim(period_id, player.clone()));

        if !participated || already_claimed || !pool.distributed {
            return EligibilityInfo {
                is_eligible: false,
                share_amount: 0,
            };
        }

        let token_address = env.storage().instance().get::<_, Address>(&DataKey::GovernanceToken).unwrap();
        let token_client = token::Client::new(&env, &token_address);
        let balance = token_client.balance(&player);

        if balance <= 0 {
            return EligibilityInfo {
                is_eligible: false,
                share_amount: 0,
            };
        }

        let participant_count = pool.eligible_participants.len();
        let share = if participant_count > 0 {
            pool.balance / participant_count as i128
        } else {
            0
        };

        EligibilityInfo {
            is_eligible: true,
            share_amount: share,
        }
    }

    // Helper functions

    fn require_init(env: &Env) {
        if !env.storage().instance().has(&DataKey::Initialized) {
            panic!("Not initialized");
        }
    }

    fn require_admin(env: &Env) {
        let admin = env.storage().instance().get::<_, Address>(&DataKey::Admin).unwrap();
        admin.require_auth();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
    use soroban_sdk::token::StellarAssetClient;

    fn create_token_contract(env: &Env, admin: &Address) -> Address {
        env.register_stellar_asset_contract_v2(admin.clone()).address()
    }

    fn setup() -> (Env, Address, Address, StellarAssetClient<'static>, SubscriptionDaoClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let token_admin = Address::generate(&env);
        let governance_token = create_token_contract(&env, &token_admin);

        let token_admin_client = StellarAssetClient::new(&env, &governance_token);

        let contract_id = env.register_contract(None, SubscriptionDao);
        let client = SubscriptionDaoClient::new(&env, &contract_id);

        client.initialize(&admin, &governance_token);

        (env, admin.clone(), governance_token, token_admin_client, client, admin)
    }

    #[test]
    fn test_initialize() {
        let (env, admin, governance_token, _token_admin_client, _client, _admin) = setup();
        let contract_id = env.register_contract(None, SubscriptionDao);
        let client = SubscriptionDaoClient::new(&env, &contract_id);

        client.initialize(&admin, &governance_token);

        let period = client.get_period(&1);
        assert_eq!(period.period_id, 1);
        assert_eq!(period.balance, 0);
        assert_eq!(period.distributed, false);
    }

    #[test]
    fn test_deposit_revenue() {
        let (env, _admin, _governance_token, token_admin_client, client, _admin_addr) = setup();

        client.deposit_revenue(&1_000_000);

        let period = client.get_period(&1);
        assert_eq!(period.balance, 1_000_000);
    }

    #[test]
    fn test_record_participation() {
        let (env, _admin, _governance_token, _token_admin_client, client, _admin_addr) = setup();

        let player = Address::generate(&env);
        client.record_participation(&player);

        let period = client.get_period(&1);
        assert_eq!(period.eligible_participants.len(), 1);
        assert_eq!(period.eligible_participants.get(0).unwrap(), player);
    }

    #[test]
    fn test_close_period() {
        let (env, admin, _governance_token, _token_admin_client, client, _admin_addr) = setup();

        env.ledger().set(LedgerInfo {
            protocol_version: 20,
            sequence_number: 10,
            timestamp: 1000,
            network_id: Default::default(),
            base_reserve: 100,
            min_persistent_entry_ttl: 1000,
            min_temp_entry_ttl: 100,
            max_entry_ttl: 100000,
        });

        client.close_period(&admin);

        let period = client.get_period(&1);
        assert!(period.distributed);
        assert!(period.snapshot_at > 0);

        let next_period = client.get_period(&2);
        assert_eq!(next_period.period_id, 2);
        assert!(!next_period.distributed);
    }

    #[test]
    fn test_claim_period_share() {
        let (env, _admin, _governance_token, token_admin_client, client, admin) = setup();

        // Deposit revenue
        client.deposit_revenue(&1_000_000);

        // Record participation
        let player = Address::generate(&env);
        token_admin_client.mint(&player, &100);
        client.record_participation(&player);

        // Close period
        client.close_period(&admin);

        // Claim share
        client.claim_period_share(&1, &player);

        let eligibility = client.get_player_eligibility(&1, &player);
        assert!(!eligibility.is_eligible); // Already claimed
    }

    #[test]
    #[should_panic(expected = "Not eligible: did not participate")]
    fn test_ineligible_claim_rejection() {
        let (env, _admin, _governance_token, token_admin_client, client, admin) = setup();

        // Deposit revenue
        client.deposit_revenue(&1_000_000);

        // Close period without recording participation
        client.close_period(&admin);

        // Try to claim without participation
        let player = Address::generate(&env);
        token_admin_client.mint(&player, &100);
        client.claim_period_share(&1, &player);
    }

    #[test]
    #[should_panic(expected = "Already claimed")]
    fn test_double_claim_prevention() {
        let (env, _admin, _governance_token, token_admin_client, client, admin) = setup();

        // Deposit revenue
        client.deposit_revenue(&1_000_000);

        // Record participation
        let player = Address::generate(&env);
        token_admin_client.mint(&player, &100);
        client.record_participation(&player);

        // Close period
        client.close_period(&admin);

        // Claim share
        client.claim_period_share(&1, &player);

        // Try to claim again
        client.claim_period_share(&1, &player);
    }

    #[test]
    fn test_get_player_eligibility() {
        let (env, _admin, _governance_token, token_admin_client, client, admin) = setup();

        // Deposit revenue
        client.deposit_revenue(&1_000_000);

        // Record participation
        let player = Address::generate(&env);
        token_admin_client.mint(&player, &100);
        client.record_participation(&player);

        // Close period
        client.close_period(&admin);

        let eligibility = client.get_player_eligibility(&1, &player);
        assert!(eligibility.is_eligible);
        assert_eq!(eligibility.share_amount, 1_000_000);
    }
}
