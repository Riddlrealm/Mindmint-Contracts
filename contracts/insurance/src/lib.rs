#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, String, Vec};

//
// ──────────────────────────────────────────────────────────
// DATA KEYS
// ──────────────────────────────────────────────────────────
//

#[contracttype]
pub enum DataKey {
    Config,                    // InsuranceConfig
    Policy(Address),           // InsurancePolicy for user
    PolicyList,                // Vec<Address> of all policyholders
    Claim(u64),                // Claim by ID
    ClaimCounter,              // u64 counter for generating claim IDs
    UserClaims(Address),       // Vec<u64> of user's claim IDs
    PremiumPool,               // i128 total premium pool
    TotalPolicies,             // u64 counter
    TotalClaims,               // u64 counter
    FraudFlags(Address),       // FraudMetrics per user
}

//
// ──────────────────────────────────────────────────────────
// ENUMS
// ──────────────────────────────────────────────────────────
//

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageType {
    NFT = 1,
    Token = 2,
    Combined = 3,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyStatus {
    Active = 1,
    Expired = 2,
    Cancelled = 3,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimStatus {
    Submitted = 1,
    UnderReview = 2,
    Approved = 3,
    Rejected = 4,
    Paid = 5,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetType {
    NFT = 1,
    Token = 2,
}

//
// ──────────────────────────────────────────────────────────
// STRUCTS
// ──────────────────────────────────────────────────────────
//

#[contracttype]
#[derive(Clone, Debug)]
pub struct InsuranceConfig {
    pub admin: Address,
    pub payment_token: Address,        // Token used for premiums/payouts
    pub base_premium_rate: u32,        // In basis points (100 = 1%)
    pub nft_multiplier: u32,           // Rate multiplier for NFT coverage
    pub token_multiplier: u32,         // Rate multiplier for token coverage
    pub combined_multiplier: u32,      // Rate multiplier for combined coverage
    pub min_coverage_period: u64,      // Minimum coverage period in seconds
    pub max_coverage_period: u64,      // Maximum coverage period in seconds
    pub max_coverage_amount: i128,     // Maximum coverage amount
    pub claim_review_period: u64,      // Time for admin to review claims
    pub max_claims_per_period: u32,    // Fraud detection: max claims per 30 days
    pub claim_cooldown: u64,           // Fraud detection: time between claims
    pub paused: bool,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct InsurancePolicy {
    pub owner: Address,
    pub coverage_type: CoverageType,
    pub coverage_amount: i128,
    pub premium_paid: i128,
    pub start_time: u64,
    pub end_time: u64,
    pub status: PolicyStatus,
    pub asset_address: Address,        // NFT contract or token address
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Claim {
    pub claim_id: u64,
    pub policy_owner: Address,
    pub asset_type: AssetType,
    pub asset_address: Address,        // Contract address of lost asset
    pub claim_amount: i128,
    pub description: String,           // Max 200 chars
    pub submission_time: u64,
    pub status: ClaimStatus,
    pub review_notes: String,          // Review notes from admin
    pub payout_amount: i128,
    pub payout_time: u64,              // 0 if not paid yet
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct FraudMetrics {
    pub total_claims: u32,
    pub recent_claims: Vec<u64>,       // Claim IDs in last 30 days
    pub last_claim_time: u64,
    pub flagged: bool,
    pub flag_reason: String,
}

//
// ──────────────────────────────────────────────────────────
// CONSTANTS
// ──────────────────────────────────────────────────────────
//

const SECONDS_PER_DAY: u64 = 86_400;
const BASIS_POINTS: u64 = 10_000;
const FRAUD_LOOKBACK_PERIOD: u64 = 30 * SECONDS_PER_DAY; // 30 days

//
// ──────────────────────────────────────────────────────────
// CONTRACT
// ──────────────────────────────────────────────────────────
//

#[contract]
pub struct InsuranceContract;

#[contractimpl]
impl InsuranceContract {
    // ───────────── INITIALIZATION ─────────────

    /// Initialize the insurance contract
    ///
    /// # Arguments
    /// * `admin` - Contract administrator
    /// * `payment_token` - Token address for premiums and payouts
    /// * `base_premium_rate` - Base premium rate in basis points (e.g., 100 = 1%)
    pub fn initialize(
        env: Env,
        admin: Address,
        payment_token: Address,
        base_premium_rate: u32,
    ) {
        admin.require_auth();

        if env.storage().persistent().has(&DataKey::Config) {
            panic!("Already initialized");
        }

        let config = InsuranceConfig {
            admin,
            payment_token,
            base_premium_rate,
            nft_multiplier: 150,              // 1.5x for NFT coverage
            token_multiplier: 100,            // 1.0x for token coverage
            combined_multiplier: 180,         // 1.8x for combined coverage
            min_coverage_period: 7 * SECONDS_PER_DAY,     // 7 days minimum
            max_coverage_period: 365 * SECONDS_PER_DAY,   // 1 year maximum
            max_coverage_amount: 1_000_000_000_000,       // 1M tokens max
            claim_review_period: 7 * SECONDS_PER_DAY,     // 7 days review time
            max_claims_per_period: 3,         // Max 3 claims per 30 days
            claim_cooldown: 7 * SECONDS_PER_DAY,          // 7 days between claims
            paused: false,
        };

        env.storage().persistent().set(&DataKey::Config, &config);
        env.storage().persistent().set(&DataKey::PremiumPool, &0i128);
        env.storage().persistent().set(&DataKey::ClaimCounter, &0u64);
        env.storage().persistent().set(&DataKey::TotalPolicies, &0u64);
        env.storage().persistent().set(&DataKey::TotalClaims, &0u64);
    }

    // ───────────── POLICY MANAGEMENT ─────────────

    /// Purchase an insurance policy
    ///
    /// # Arguments
    /// * `owner` - Policy owner
    /// * `coverage_type` - Type of coverage (NFT, Token, or Combined)
    /// * `coverage_amount` - Amount of coverage
    /// * `coverage_period` - Coverage period in seconds
    /// * `asset_address` - Address of the asset to insure
    pub fn purchase_policy(
        env: Env,
        owner: Address,
        coverage_type: CoverageType,
        coverage_amount: i128,
        coverage_period: u64,
        asset_address: Address,
    ) {
        owner.require_auth();
        Self::assert_not_paused(&env);

        let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();

        // Validations
        if coverage_amount <= 0 || coverage_amount > config.max_coverage_amount {
            panic!("Invalid coverage amount");
        }

        if coverage_period < config.min_coverage_period || coverage_period > config.max_coverage_period {
            panic!("Invalid coverage period");
        }

        // Check if user already has an active policy
        if let Some(existing_policy) = Self::get_policy(env.clone(), owner.clone()) {
            if existing_policy.status == PolicyStatus::Active {
                panic!("User already has an active policy");
            }
        }

        // Calculate premium
        let premium = Self::calculate_premium_internal(
            &env,
            &config,
            coverage_type,
            coverage_amount,
            coverage_period,
        );

        // Transfer premium from user to contract
        let token_client = token::Client::new(&env, &config.payment_token);
        token_client.transfer(&owner, &env.current_contract_address(), &premium);

        // Create policy
        let start_time = env.ledger().timestamp();
        let end_time = start_time + coverage_period;

        let policy = InsurancePolicy {
            owner: owner.clone(),
            coverage_type,
            coverage_amount,
            premium_paid: premium,
            start_time,
            end_time,
            status: PolicyStatus::Active,
            asset_address,
        };

        // Store policy
        env.storage().persistent().set(&DataKey::Policy(owner.clone()), &policy);

        // Add to policy list
        Self::add_to_policy_list(&env, owner);

        // Update premium pool
        let pool: i128 = env.storage().persistent().get(&DataKey::PremiumPool).unwrap_or(0);
        env.storage().persistent().set(&DataKey::PremiumPool, &(pool + premium));

        // Increment total policies
        let total: u64 = env.storage().persistent().get(&DataKey::TotalPolicies).unwrap_or(0);
        env.storage().persistent().set(&DataKey::TotalPolicies, &(total + 1));
    }

    /// Renew an existing policy
    ///
    /// # Arguments
    /// * `owner` - Policy owner
    /// * `additional_period` - Additional coverage period in seconds
    pub fn renew_policy(env: Env, owner: Address, additional_period: u64) {
        owner.require_auth();
        Self::assert_not_paused(&env);

        let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let mut policy: InsurancePolicy = env.storage().persistent()
            .get(&DataKey::Policy(owner.clone()))
            .expect("Policy not found");

        // Validations
        if policy.status != PolicyStatus::Active && policy.status != PolicyStatus::Expired {
            panic!("Policy cannot be renewed");
        }

        let current_time = env.ledger().timestamp();
        let new_end_time = if policy.end_time > current_time {
            policy.end_time + additional_period
        } else {
            current_time + additional_period
        };

        let total_period = new_end_time - policy.start_time;
        if total_period > config.max_coverage_period {
            panic!("Total coverage period exceeds maximum");
        }

        // Calculate additional premium
        let additional_premium = Self::calculate_premium_internal(
            &env,
            &config,
            policy.coverage_type,
            policy.coverage_amount,
            additional_period,
        );

        // Transfer premium from user to contract
        let token_client = token::Client::new(&env, &config.payment_token);
        token_client.transfer(&owner, &env.current_contract_address(), &additional_premium);

        // Update policy
        policy.end_time = new_end_time;
        policy.premium_paid += additional_premium;
        policy.status = PolicyStatus::Active;

        env.storage().persistent().set(&DataKey::Policy(owner), &policy);

        // Update premium pool
        let pool: i128 = env.storage().persistent().get(&DataKey::PremiumPool).unwrap_or(0);
        env.storage().persistent().set(&DataKey::PremiumPool, &(pool + additional_premium));
    }

    /// Cancel a policy and receive prorated refund
    ///
    /// # Arguments
    /// * `owner` - Policy owner
    pub fn cancel_policy(env: Env, owner: Address) {
        owner.require_auth();

        let mut policy: InsurancePolicy = env.storage().persistent()
            .get(&DataKey::Policy(owner.clone()))
            .expect("Policy not found");

        if policy.status != PolicyStatus::Active {
            panic!("Policy is not active");
        }

        let current_time = env.ledger().timestamp();
        let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();

        // Calculate refund (prorated based on unused time)
        let total_period = policy.end_time - policy.start_time;
        let _elapsed_period = current_time - policy.start_time;
        let remaining_period = if policy.end_time > current_time {
            policy.end_time - current_time
        } else {
            0
        };

        let refund = if remaining_period > 0 {
            (policy.premium_paid * remaining_period as i128) / total_period as i128
        } else {
            0
        };

        // Update policy status
        policy.status = PolicyStatus::Cancelled;
        env.storage().persistent().set(&DataKey::Policy(owner.clone()), &policy);

        // Process refund if applicable
        if refund > 0 {
            let token_client = token::Client::new(&env, &config.payment_token);
            token_client.transfer(&env.current_contract_address(), &owner, &refund);

            // Update premium pool
            let pool: i128 = env.storage().persistent().get(&DataKey::PremiumPool).unwrap_or(0);
            env.storage().persistent().set(&DataKey::PremiumPool, &(pool - refund));
        }
    }

    // ───────────── CLAIM MANAGEMENT ─────────────

    /// Submit an insurance claim
    ///
    /// # Arguments
    /// * `claimant` - User submitting the claim
    /// * `asset_type` - Type of asset (NFT or Token)
    /// * `asset_address` - Address of the lost asset
    /// * `claim_amount` - Amount being claimed
    /// * `description` - Description of the claim
    ///
    /// # Returns
    /// * Claim ID
    pub fn submit_claim(
        env: Env,
        claimant: Address,
        asset_type: AssetType,
        asset_address: Address,
        claim_amount: i128,
        description: String,
    ) -> u64 {
        claimant.require_auth();
        Self::assert_not_paused(&env);

        // Get policy
        let policy: InsurancePolicy = env.storage().persistent()
            .get(&DataKey::Policy(claimant.clone()))
            .expect("No active policy found");

        // Validations
        let current_time = env.ledger().timestamp();

        // Check policy is active
        if policy.status != PolicyStatus::Active {
            panic!("Policy is not active");
        }

        // Check within coverage period
        if current_time < policy.start_time || current_time > policy.end_time {
            panic!("Outside coverage period");
        }

        // Check coverage type matches asset type
        match (policy.coverage_type, asset_type) {
            (CoverageType::NFT, AssetType::Token) => panic!("Policy does not cover tokens"),
            (CoverageType::Token, AssetType::NFT) => panic!("Policy does not cover NFTs"),
            _ => {}
        }

        // Check claim amount
        if claim_amount <= 0 || claim_amount > policy.coverage_amount {
            panic!("Invalid claim amount");
        }

        // Fraud checks
        Self::check_fraud(&env, &claimant);

        // Generate claim ID
        let claim_id: u64 = env.storage().persistent().get(&DataKey::ClaimCounter).unwrap_or(0);
        let new_claim_id = claim_id + 1;
        env.storage().persistent().set(&DataKey::ClaimCounter, &new_claim_id);

        // Create claim
        let claim = Claim {
            claim_id: new_claim_id,
            policy_owner: claimant.clone(),
            asset_type,
            asset_address,
            claim_amount,
            description,
            submission_time: current_time,
            status: ClaimStatus::Submitted,
            review_notes: String::from_str(&env, ""),
            payout_amount: 0,
            payout_time: 0,
        };

        // Store claim
        env.storage().persistent().set(&DataKey::Claim(new_claim_id), &claim);

        // Add to user's claims list
        Self::add_to_user_claims(&env, claimant.clone(), new_claim_id);

        // Update fraud metrics
        Self::update_fraud_metrics(&env, claimant, new_claim_id, current_time);

        // Increment total claims
        let total: u64 = env.storage().persistent().get(&DataKey::TotalClaims).unwrap_or(0);
        env.storage().persistent().set(&DataKey::TotalClaims, &(total + 1));

        new_claim_id
    }

    /// Review a claim (admin only)
    ///
    /// # Arguments
    /// * `admin` - Admin address
    /// * `claim_id` - Claim ID to review
    /// * `approved` - Whether claim is approved
    /// * `review_notes` - Review notes
    /// * `payout_amount` - Approved payout amount (if approved)
    pub fn review_claim(
        env: Env,
        admin: Address,
        claim_id: u64,
        approved: bool,
        review_notes: String,
        payout_amount: i128,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut claim: Claim = env.storage().persistent()
            .get(&DataKey::Claim(claim_id))
            .expect("Claim not found");

        if claim.status != ClaimStatus::Submitted && claim.status != ClaimStatus::UnderReview {
            panic!("Claim cannot be reviewed");
        }

        if approved {
            if payout_amount <= 0 || payout_amount > claim.claim_amount {
                panic!("Invalid payout amount");
            }
            claim.status = ClaimStatus::Approved;
            claim.payout_amount = payout_amount;
        } else {
            claim.status = ClaimStatus::Rejected;
            claim.payout_amount = 0;
        }

        claim.review_notes = review_notes;

        env.storage().persistent().set(&DataKey::Claim(claim_id), &claim);
    }

    /// Process payout for an approved claim (admin only)
    ///
    /// # Arguments
    /// * `admin` - Admin address
    /// * `claim_id` - Claim ID to process
    pub fn process_payout(env: Env, admin: Address, claim_id: u64) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut claim: Claim = env.storage().persistent()
            .get(&DataKey::Claim(claim_id))
            .expect("Claim not found");

        if claim.status != ClaimStatus::Approved {
            panic!("Claim is not approved");
        }

        let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let pool: i128 = env.storage().persistent().get(&DataKey::PremiumPool).unwrap_or(0);

        // Check pool has sufficient funds
        if pool < claim.payout_amount {
            panic!("Insufficient premium pool");
        }

        // Transfer payout to claimant
        let token_client = token::Client::new(&env, &config.payment_token);
        token_client.transfer(
            &env.current_contract_address(),
            &claim.policy_owner,
            &claim.payout_amount,
        );

        // Update claim
        claim.status = ClaimStatus::Paid;
        claim.payout_time = env.ledger().timestamp();
        env.storage().persistent().set(&DataKey::Claim(claim_id), &claim);

        // Update premium pool
        env.storage().persistent().set(&DataKey::PremiumPool, &(pool - claim.payout_amount));
    }

    // ───────────── PREMIUM POOL MANAGEMENT ─────────────

    /// Add funds to premium pool (admin only)
    ///
    /// # Arguments
    /// * `admin` - Admin address
    /// * `amount` - Amount to add
    pub fn add_to_pool(env: Env, admin: Address, amount: i128) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if amount <= 0 {
            panic!("Amount must be positive");
        }

        let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let token_client = token::Client::new(&env, &config.payment_token);

        token_client.transfer(&admin, &env.current_contract_address(), &amount);

        let pool: i128 = env.storage().persistent().get(&DataKey::PremiumPool).unwrap_or(0);
        env.storage().persistent().set(&DataKey::PremiumPool, &(pool + amount));
    }

    /// Withdraw from premium pool (admin only)
    ///
    /// # Arguments
    /// * `admin` - Admin address
    /// * `amount` - Amount to withdraw
    pub fn withdraw_from_pool(env: Env, admin: Address, amount: i128) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if amount <= 0 {
            panic!("Amount must be positive");
        }

        let pool: i128 = env.storage().persistent().get(&DataKey::PremiumPool).unwrap_or(0);

        if pool < amount {
            panic!("Insufficient pool balance");
        }

        let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let token_client = token::Client::new(&env, &config.payment_token);

        token_client.transfer(&env.current_contract_address(), &admin, &amount);

        env.storage().persistent().set(&DataKey::PremiumPool, &(pool - amount));
    }

    // ───────────── FRAUD MANAGEMENT ─────────────

    /// Flag a user for suspicious activity (admin only)
    ///
    /// # Arguments
    /// * `admin` - Admin address
    /// * `user` - User to flag
    /// * `reason` - Reason for flagging
    pub fn flag_user(env: Env, admin: Address, user: Address, reason: String) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut metrics = Self::get_fraud_metrics(env.clone(), user.clone())
            .unwrap_or(FraudMetrics {
                total_claims: 0,
                recent_claims: Vec::new(&env),
                last_claim_time: 0,
                flagged: false,
                flag_reason: String::from_str(&env, ""),
            });

        metrics.flagged = true;
        metrics.flag_reason = reason;

        env.storage().persistent().set(&DataKey::FraudFlags(user), &metrics);
    }

    /// Unflag a user (admin only)
    ///
    /// # Arguments
    /// * `admin` - Admin address
    /// * `user` - User to unflag
    pub fn unflag_user(env: Env, admin: Address, user: Address) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if let Some(mut metrics) = Self::get_fraud_metrics(env.clone(), user.clone()) {
            metrics.flagged = false;
            metrics.flag_reason = String::from_str(&env, "");
            env.storage().persistent().set(&DataKey::FraudFlags(user), &metrics);
        }
    }

    // ───────────── VIEW FUNCTIONS ─────────────

    /// Get policy information
    pub fn get_policy(env: Env, user: Address) -> Option<InsurancePolicy> {
        env.storage().persistent().get(&DataKey::Policy(user))
    }

    /// Get claim information
    pub fn get_claim(env: Env, claim_id: u64) -> Option<Claim> {
        env.storage().persistent().get(&DataKey::Claim(claim_id))
    }

    /// Get user's claim history
    pub fn get_user_claims(env: Env, user: Address) -> Vec<u64> {
        env.storage().persistent()
            .get(&DataKey::UserClaims(user))
            .unwrap_or(Vec::new(&env))
    }

    /// Get all policies
    pub fn get_all_policies(env: Env) -> Vec<Address> {
        env.storage().persistent()
            .get(&DataKey::PolicyList)
            .unwrap_or(Vec::new(&env))
    }

    /// Get total policies count
    pub fn get_total_policies(env: Env) -> u64 {
        env.storage().persistent().get(&DataKey::TotalPolicies).unwrap_or(0)
    }

    /// Get total claims count
    pub fn get_total_claims(env: Env) -> u64 {
        env.storage().persistent().get(&DataKey::TotalClaims).unwrap_or(0)
    }

    /// Check if policy is active
    pub fn is_policy_active(env: Env, user: Address) -> bool {
        if let Some(policy) = Self::get_policy(env.clone(), user) {
            let current_time = env.ledger().timestamp();
            policy.status == PolicyStatus::Active 
                && current_time >= policy.start_time 
                && current_time <= policy.end_time
        } else {
            false
        }
    }

    /// Get premium pool balance
    pub fn get_premium_pool(env: Env) -> i128 {
        env.storage().persistent().get(&DataKey::PremiumPool).unwrap_or(0)
    }

    /// Get configuration
    pub fn get_config(env: Env) -> InsuranceConfig {
        env.storage().persistent().get(&DataKey::Config).unwrap()
    }

    /// Get fraud metrics for a user
    pub fn get_fraud_metrics(env: Env, user: Address) -> Option<FraudMetrics> {
        env.storage().persistent().get(&DataKey::FraudFlags(user))
    }

    /// Calculate premium for given parameters
    pub fn calculate_premium(
        env: Env,
        coverage_type: CoverageType,
        coverage_amount: i128,
        coverage_period: u64,
    ) -> i128 {
        let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        Self::calculate_premium_internal(&env, &config, coverage_type, coverage_amount, coverage_period)
    }

    // ───────────── ADMIN FUNCTIONS ─────────────

    /// Update premium rates (admin only)
    pub fn update_premium_rates(
        env: Env,
        admin: Address,
        base_rate: u32,
        nft_mult: u32,
        token_mult: u32,
        combined_mult: u32,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();

        config.base_premium_rate = base_rate;
        config.nft_multiplier = nft_mult;
        config.token_multiplier = token_mult;
        config.combined_multiplier = combined_mult;

        env.storage().persistent().set(&DataKey::Config, &config);
    }

    /// Update coverage limits (admin only)
    pub fn update_coverage_limits(
        env: Env,
        admin: Address,
        min_period: u64,
        max_period: u64,
        max_amount: i128,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();

        config.min_coverage_period = min_period;
        config.max_coverage_period = max_period;
        config.max_coverage_amount = max_amount;

        env.storage().persistent().set(&DataKey::Config, &config);
    }

    /// Update fraud detection parameters (admin only)
    pub fn update_fraud_params(
        env: Env,
        admin: Address,
        max_claims: u32,
        cooldown: u64,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();

        config.max_claims_per_period = max_claims;
        config.claim_cooldown = cooldown;

        env.storage().persistent().set(&DataKey::Config, &config);
    }

    /// Pause/unpause contract (admin only)
    pub fn set_paused(env: Env, admin: Address, paused: bool) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        config.paused = paused;
        env.storage().persistent().set(&DataKey::Config, &config);
    }

    /// Emergency withdrawal of entire pool (admin only)
    pub fn emergency_withdraw(env: Env, admin: Address) -> i128 {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let pool: i128 = env.storage().persistent().get(&DataKey::PremiumPool).unwrap_or(0);

        if pool > 0 {
            let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
            let token_client = token::Client::new(&env, &config.payment_token);

            token_client.transfer(&env.current_contract_address(), &admin, &pool);

            env.storage().persistent().set(&DataKey::PremiumPool, &0i128);
        }

        pool
    }

    // ───────────── INTERNAL HELPERS ─────────────

    fn calculate_premium_internal(
        _env: &Env,
        config: &InsuranceConfig,
        coverage_type: CoverageType,
        coverage_amount: i128,
        coverage_period: u64,
    ) -> i128 {
        // Get multiplier based on coverage type
        let multiplier = match coverage_type {
            CoverageType::NFT => config.nft_multiplier,
            CoverageType::Token => config.token_multiplier,
            CoverageType::Combined => config.combined_multiplier,
        };

        // Calculate: coverage_amount * base_rate * multiplier * (period_days / 365) / (BASIS_POINTS * 100)
        let coverage_days = coverage_period / SECONDS_PER_DAY;
        let annual_rate = (config.base_premium_rate as i128 * multiplier as i128) / 100; // Divide by 100 for multiplier percentage

        // Premium = coverage_amount * annual_rate * (coverage_days / 365) / BASIS_POINTS
        let premium = (coverage_amount * annual_rate * coverage_days as i128) 
            / (365 * BASIS_POINTS as i128);

        // Ensure minimum premium of 1
        if premium < 1 {
            1
        } else {
            premium
        }
    }

    fn check_fraud(env: &Env, user: &Address) {
        let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let current_time = env.ledger().timestamp();

        // Get or create fraud metrics
        let metrics = env.storage().persistent()
            .get::<DataKey, FraudMetrics>(&DataKey::FraudFlags(user.clone()))
            .unwrap_or(FraudMetrics {
                total_claims: 0,
                recent_claims: Vec::new(env),
                last_claim_time: 0,
                flagged: false,
                flag_reason: String::from_str(env, ""),
            });

        // Check if user is flagged
        if metrics.flagged {
            panic!("User is flagged for suspicious activity");
        }

        // Check claim cooldown
        if metrics.last_claim_time > 0 {
            let time_since_last = current_time - metrics.last_claim_time;
            if time_since_last < config.claim_cooldown {
                panic!("Claim submitted too soon after previous claim");
            }
        }

        // Check recent claim frequency
        let lookback_time = if current_time > FRAUD_LOOKBACK_PERIOD {
            current_time - FRAUD_LOOKBACK_PERIOD
        } else {
            0
        };

        let mut recent_count = 0u32;
        for claim_id in metrics.recent_claims.iter() {
            if let Some(claim) = env.storage().persistent().get::<DataKey, Claim>(&DataKey::Claim(claim_id)) {
                if claim.submission_time >= lookback_time {
                    recent_count += 1;
                }
            }
        }

        if recent_count >= config.max_claims_per_period {
            panic!("Too many claims in recent period");
        }
    }

    fn update_fraud_metrics(env: &Env, user: Address, claim_id: u64, current_time: u64) {
        let mut metrics = env.storage().persistent()
            .get::<DataKey, FraudMetrics>(&DataKey::FraudFlags(user.clone()))
            .unwrap_or(FraudMetrics {
                total_claims: 0,
                recent_claims: Vec::new(env),
                last_claim_time: 0,
                flagged: false,
                flag_reason: String::from_str(env, ""),
            });

        metrics.total_claims += 1;
        metrics.last_claim_time = current_time;

        // Add to recent claims, removing old ones
        let lookback_time = if current_time > FRAUD_LOOKBACK_PERIOD {
            current_time - FRAUD_LOOKBACK_PERIOD
        } else {
            0
        };

        let mut new_recent: Vec<u64> = Vec::new(env);
        for id in metrics.recent_claims.iter() {
            if let Some(claim) = env.storage().persistent().get::<DataKey, Claim>(&DataKey::Claim(id)) {
                if claim.submission_time >= lookback_time {
                    new_recent.push_back(id);
                }
            }
        }
        new_recent.push_back(claim_id);

        metrics.recent_claims = new_recent;

        env.storage().persistent().set(&DataKey::FraudFlags(user), &metrics);
    }

    fn add_to_policy_list(env: &Env, user: Address) {
        let mut policies: Vec<Address> = env.storage().persistent()
            .get(&DataKey::PolicyList)
            .unwrap_or(Vec::new(env));

        if !policies.contains(&user) {
            policies.push_back(user);
            env.storage().persistent().set(&DataKey::PolicyList, &policies);
        }
    }

    fn add_to_user_claims(env: &Env, user: Address, claim_id: u64) {
        let mut claims: Vec<u64> = env.storage().persistent()
            .get(&DataKey::UserClaims(user.clone()))
            .unwrap_or(Vec::new(env));

        claims.push_back(claim_id);
        env.storage().persistent().set(&DataKey::UserClaims(user), &claims);
    }

    fn assert_admin(env: &Env, user: &Address) {
        let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        if config.admin != *user {
            panic!("Admin only");
        }
    }

    fn assert_not_paused(env: &Env) {
        let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        if config.paused {
            panic!("Contract is paused");
        }
    }
}

mod test;
