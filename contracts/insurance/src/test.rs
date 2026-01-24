#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::Client as TokenClient,
    token::StellarAssetClient,
    Address, Env, String,
};

fn create_token_contract<'a>(env: &Env, admin: &Address) -> (Address, TokenClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let address = sac.address();
    (address.clone(), TokenClient::new(env, &address))
}

fn setup_insurance_contract(env: &Env) -> (
    InsuranceContractClient,
    Address,
    Address,
    Address,
    TokenClient,
    StellarAssetClient,
) {
    let admin = Address::generate(env);
    let user = Address::generate(env);
    let token_admin = Address::generate(env);

    // Create payment token
    let (payment_token_addr, payment_token_client) = create_token_contract(env, &token_admin);
    let payment_admin_client = StellarAssetClient::new(env, &payment_token_addr);

    // Register insurance contract
    let contract_id = env.register_contract(None, InsuranceContract);
    let client = InsuranceContractClient::new(env, &contract_id);

    // Initialize with 1% base rate (100 basis points)
    let base_rate = 100u32;

    client.initialize(&admin, &payment_token_addr, &base_rate);

    (
        client,
        admin,
        user,
        token_admin,
        payment_token_client,
        payment_admin_client,
    )
}

// ───────────── INITIALIZATION TESTS ─────────────

#[test]
fn test_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, _, _) = setup_insurance_contract(&env);

    let config = client.get_config();
    assert_eq!(config.admin, admin);
    assert_eq!(config.base_premium_rate, 100);
    assert_eq!(config.nft_multiplier, 150);
    assert_eq!(config.token_multiplier, 100);
    assert_eq!(config.combined_multiplier, 180);
    assert!(!config.paused);

    assert_eq!(client.get_premium_pool(), 0);
    assert_eq!(client.get_total_policies(), 0);
    assert_eq!(client.get_total_claims(), 0);
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_double_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, payment_token_client, _) = setup_insurance_contract(&env);

    // Try to initialize again
    client.initialize(&admin, &payment_token_client.address, &100u32);
}

// ───────────── POLICY PURCHASE TESTS ─────────────

#[test]
fn test_purchase_policy_token_coverage() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    let asset_addr = Address::generate(&env);
    let coverage_amount = 1_000_000_000i128; // 1,000 tokens
    let coverage_period = 30 * 86_400u64; // 30 days

    // Calculate expected premium
    let expected_premium = client.calculate_premium(
        &CoverageType::Token,
        &coverage_amount,
        &coverage_period,
    );

    // Mint tokens to user for premium payment
    payment_admin_client.mint(&user, &(expected_premium * 2));

    // Purchase policy
    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &coverage_amount,
        &coverage_period,
        &asset_addr,
    );

    // Verify policy was created
    let policy = client.get_policy(&user).unwrap();
    assert_eq!(policy.owner, user);
    assert_eq!(policy.coverage_type, CoverageType::Token);
    assert_eq!(policy.coverage_amount, coverage_amount);
    assert_eq!(policy.premium_paid, expected_premium);
    assert_eq!(policy.status, PolicyStatus::Active);
    assert_eq!(policy.start_time, 1000);
    assert_eq!(policy.end_time, 1000 + coverage_period);

    // Verify premium pool updated
    assert_eq!(client.get_premium_pool(), expected_premium);
    assert_eq!(client.get_total_policies(), 1);
}

#[test]
fn test_purchase_policy_nft_coverage() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    let nft_addr = Address::generate(&env);
    let coverage_amount = 500_000_000i128;
    let coverage_period = 90 * 86_400u64; // 90 days

    let expected_premium = client.calculate_premium(
        &CoverageType::NFT,
        &coverage_amount,
        &coverage_period,
    );

    payment_admin_client.mint(&user, &(expected_premium * 2));

    client.purchase_policy(
        &user,
        &CoverageType::NFT,
        &coverage_amount,
        &coverage_period,
        &nft_addr,
    );

    let policy = client.get_policy(&user).unwrap();
    assert_eq!(policy.coverage_type, CoverageType::NFT);
    
    // NFT coverage should have higher premium (1.5x multiplier)
    assert!(policy.premium_paid > 0);
}

#[test]
fn test_premium_calculation() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, _, _, _, _) = setup_insurance_contract(&env);

    let coverage_amount = 1_000_000_000i128;
    let coverage_period = 365 * 86_400u64; // 1 year

    // Token coverage: 1% base rate * 1.0x = 1%
    let token_premium = client.calculate_premium(
        &CoverageType::Token,
        &coverage_amount,
        &coverage_period,
    );
    // Expected: 1,000,000,000 * 100 * 100 / (365 * 10000) = ~27,397
    assert!(token_premium > 0);

    // NFT coverage: 1% base rate * 1.5x = 1.5%
    let nft_premium = client.calculate_premium(
        &CoverageType::NFT,
        &coverage_amount,
        &coverage_period,
    );
    // Should be 1.5x the token premium
    assert!(nft_premium > token_premium);

    // Combined coverage: 1% base rate * 1.8x = 1.8%
    let combined_premium = client.calculate_premium(
        &CoverageType::Combined,
        &coverage_amount,
        &coverage_period,
    );
    // Should be 1.8x the token premium
    assert!(combined_premium > nft_premium);
}

#[test]
#[should_panic(expected = "Invalid coverage amount")]
fn test_purchase_policy_exceeds_max_coverage() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, user, _, _, _) = setup_insurance_contract(&env);

    let config = client.get_config();
    let excessive_amount = config.max_coverage_amount + 1;

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &excessive_amount,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );
}

#[test]
#[should_panic(expected = "Invalid coverage period")]
fn test_purchase_policy_period_too_short() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, user, _, _, _) = setup_insurance_contract(&env);

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(1 * 86_400u64), // 1 day - too short
        &Address::generate(&env),
    );
}

#[test]
#[should_panic(expected = "User already has an active policy")]
fn test_cannot_purchase_multiple_active_policies() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    // Purchase first policy
    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );

    // Try to purchase second policy (should fail)
    client.purchase_policy(
        &user,
        &CoverageType::NFT,
        &500_000_000i128,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );
}

// ───────────── POLICY RENEWAL TESTS ─────────────

#[test]
fn test_renew_policy() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    let coverage_amount = 1_000_000_000i128;
    let initial_period = 30 * 86_400u64;

    // Purchase policy
    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &coverage_amount,
        &initial_period,
        &Address::generate(&env),
    );

    let initial_policy = client.get_policy(&user).unwrap();
    let initial_premium = initial_policy.premium_paid;

    // Renew for another 30 days
    let additional_period = 30 * 86_400u64;
    client.renew_policy(&user, &additional_period);

    let renewed_policy = client.get_policy(&user).unwrap();
    assert_eq!(renewed_policy.end_time, 1000 + initial_period + additional_period);
    assert!(renewed_policy.premium_paid > initial_premium);
    assert_eq!(renewed_policy.status, PolicyStatus::Active);
}

#[test]
fn test_renew_expired_policy() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    let coverage_period = 30 * 86_400u64;

    // Purchase policy
    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &coverage_period,
        &Address::generate(&env),
    );

    // Fast forward past expiration
    env.ledger().set_timestamp(1000 + coverage_period + 1000);

    // Renew should still work
    client.renew_policy(&user, &(30 * 86_400u64));

    let renewed_policy = client.get_policy(&user).unwrap();
    assert_eq!(renewed_policy.status, PolicyStatus::Active);
}

// ───────────── POLICY CANCELLATION TESTS ─────────────

#[test]
fn test_cancel_policy_with_refund() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, payment_token_client, payment_admin_client) =
        setup_insurance_contract(&env);

    let coverage_period = 30 * 86_400u64;
    let coverage_amount = 1_000_000_000i128;

    let premium = client.calculate_premium(
        &CoverageType::Token,
        &coverage_amount,
        &coverage_period,
    );

    payment_admin_client.mint(&user, &premium);

    // Purchase policy
    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &coverage_amount,
        &coverage_period,
        &Address::generate(&env),
    );

    let initial_balance = payment_token_client.balance(&user);

    // Cancel after 10 days (1/3 of period used)
    env.ledger().set_timestamp(1000 + 10 * 86_400);
    client.cancel_policy(&user);

    // Should receive ~2/3 refund
    let final_balance = payment_token_client.balance(&user);
    assert!(final_balance > initial_balance);

    // Verify policy status
    let policy = client.get_policy(&user).unwrap();
    assert_eq!(policy.status, PolicyStatus::Cancelled);
}

#[test]
#[should_panic(expected = "Policy is not active")]
fn test_cannot_cancel_already_cancelled() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );

    client.cancel_policy(&user);
    client.cancel_policy(&user); // Should fail
}

// ───────────── CLAIM SUBMISSION TESTS ─────────────

#[test]
fn test_submit_claim() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    let asset_addr = Address::generate(&env);
    let coverage_amount = 1_000_000_000i128;

    // Purchase policy
    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &coverage_amount,
        &(30 * 86_400u64),
        &asset_addr.clone(),
    );

    // Submit claim
    env.ledger().set_timestamp(1000 + 10 * 86_400);
    let claim_amount = 500_000_000i128;
    let description = String::from_str(&env, "Lost tokens in hack");

    let claim_id = client.submit_claim(
        &user,
        &AssetType::Token,
        &asset_addr,
        &claim_amount,
        &description,
    );

    assert_eq!(claim_id, 1);

    // Verify claim
    let claim = client.get_claim(&claim_id).unwrap();
    assert_eq!(claim.claim_id, claim_id);
    assert_eq!(claim.policy_owner, user);
    assert_eq!(claim.asset_type, AssetType::Token);
    assert_eq!(claim.claim_amount, claim_amount);
    assert_eq!(claim.status, ClaimStatus::Submitted);

    // Verify user claims list
    let user_claims = client.get_user_claims(&user);
    assert_eq!(user_claims.len(), 1);
    assert_eq!(user_claims.get(0).unwrap(), claim_id);

    assert_eq!(client.get_total_claims(), 1);
}

#[test]
#[should_panic(expected = "No active policy found")]
fn test_submit_claim_without_policy() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, user, _, _, _) = setup_insurance_contract(&env);

    client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &1_000_000_000i128,
        &String::from_str(&env, "Test"),
    );
}

#[test]
#[should_panic(expected = "Outside coverage period")]
fn test_submit_claim_before_coverage_starts() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );

    // Try to submit claim before start time
    env.ledger().set_timestamp(999);

    client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &500_000_000i128,
        &String::from_str(&env, "Test"),
    );
}

#[test]
#[should_panic(expected = "Outside coverage period")]
fn test_submit_claim_after_coverage_ends() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    let coverage_period = 30 * 86_400u64;

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &coverage_period,
        &Address::generate(&env),
    );

    // Try to submit claim after end time
    env.ledger().set_timestamp(1000 + coverage_period + 1);

    client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &500_000_000i128,
        &String::from_str(&env, "Test"),
    );
}

#[test]
#[should_panic(expected = "Invalid claim amount")]
fn test_submit_claim_exceeds_coverage() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    let coverage_amount = 1_000_000_000i128;

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &coverage_amount,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );

    env.ledger().set_timestamp(1000 + 10 * 86_400);

    // Try to claim more than coverage
    client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &(coverage_amount + 1),
        &String::from_str(&env, "Test"),
    );
}

#[test]
#[should_panic(expected = "Policy does not cover tokens")]
fn test_submit_claim_wrong_asset_type() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    // Purchase NFT coverage
    client.purchase_policy(
        &user,
        &CoverageType::NFT,
        &1_000_000_000i128,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );

    env.ledger().set_timestamp(1000 + 10 * 86_400);

    // Try to claim for tokens (should fail)
    client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &500_000_000i128,
        &String::from_str(&env, "Test"),
    );
}

// ───────────── CLAIM REVIEW TESTS ─────────────

#[test]
fn test_review_and_approve_claim() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, admin, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );

    env.ledger().set_timestamp(1000 + 10 * 86_400);

    let claim_id = client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &500_000_000i128,
        &String::from_str(&env, "Lost tokens"),
    );

    // Admin reviews and approves
    let payout_amount = 450_000_000i128;
    client.review_claim(
        &admin,
        &claim_id,
        &true,
        &String::from_str(&env, "Approved after investigation"),
        &payout_amount,
    );

    let claim = client.get_claim(&claim_id).unwrap();
    assert_eq!(claim.status, ClaimStatus::Approved);
    assert_eq!(claim.payout_amount, payout_amount);
}

#[test]
fn test_review_and_reject_claim() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, admin, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );

    env.ledger().set_timestamp(1000 + 10 * 86_400);

    let claim_id = client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &500_000_000i128,
        &String::from_str(&env, "Lost tokens"),
    );

    // Admin rejects
    client.review_claim(
        &admin,
        &claim_id,
        &false,
        &String::from_str(&env, "Insufficient evidence"),
        &0i128,
    );

    let claim = client.get_claim(&claim_id).unwrap();
    assert_eq!(claim.status, ClaimStatus::Rejected);
    assert_eq!(claim.payout_amount, 0);
}

#[test]
#[should_panic(expected = "Admin only")]
fn test_non_admin_cannot_review() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );

    env.ledger().set_timestamp(1000 + 10 * 86_400);

    let claim_id = client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &500_000_000i128,
        &String::from_str(&env, "Test"),
    );

    // User tries to review own claim (should fail)
    client.review_claim(
        &user,
        &claim_id,
        &true,
        &String::from_str(&env, "Self-approve"),
        &500_000_000i128,
    );
}

// ───────────── CLAIM PAYOUT TESTS ─────────────

#[test]
fn test_process_payout() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, admin, user, _, payment_token_client, payment_admin_client) =
        setup_insurance_contract(&env);

    // Add funds to premium pool
    payment_admin_client.mint(&admin, &10_000_000_000i128);
    client.add_to_pool(&admin, &5_000_000_000i128);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );

    env.ledger().set_timestamp(1000 + 10 * 86_400);

    let claim_id = client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &500_000_000i128,
        &String::from_str(&env, "Lost tokens"),
    );

    let payout_amount = 450_000_000i128;
    client.review_claim(
        &admin,
        &claim_id,
        &true,
        &String::from_str(&env, "Approved"),
        &payout_amount,
    );

    let pool_before = client.get_premium_pool();
    let balance_before = payment_token_client.balance(&user);

    // Process payout
    client.process_payout(&admin, &claim_id);

    // Verify payout
    let claim = client.get_claim(&claim_id).unwrap();
    assert_eq!(claim.status, ClaimStatus::Paid);
    assert!(claim.payout_time > 0);

    // Verify balances
    let pool_after = client.get_premium_pool();
    let balance_after = payment_token_client.balance(&user);

    assert_eq!(pool_after, pool_before - payout_amount);
    assert_eq!(balance_after, balance_before + payout_amount);
}

#[test]
#[should_panic(expected = "Claim is not approved")]
fn test_cannot_payout_unapproved_claim() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, admin, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );

    env.ledger().set_timestamp(1000 + 10 * 86_400);

    let claim_id = client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &500_000_000i128,
        &String::from_str(&env, "Test"),
    );

    // Try to payout without approval
    client.process_payout(&admin, &claim_id);
}

#[test]
#[should_panic(expected = "Insufficient premium pool")]
fn test_cannot_payout_insufficient_pool() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, admin, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    // Use a higher coverage amount
    let coverage_amount = 10_000_000_000i128;
    
    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &coverage_amount,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );

    env.ledger().set_timestamp(1000 + 10 * 86_400);

    // Submit claim for most of the coverage
    let claim_amount = 8_000_000_000i128;
    let claim_id = client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &claim_amount,
        &String::from_str(&env, "Test"),
    );

    // Approve for the full claim amount (within limit, so validation passes)
    client.review_claim(
        &admin,
        &claim_id,
        &true,
        &String::from_str(&env, "Approved"),
        &claim_amount,
    );

    // Try to payout (should fail - insufficient pool)
    // Pool only has the small premium from the policy purchase, not enough for 8B payout
    client.process_payout(&admin, &claim_id);
}

// ───────────── FRAUD DETECTION TESTS ─────────────

#[test]
#[should_panic(expected = "Claim submitted too soon after previous claim")]
fn test_claim_cooldown_enforcement() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(90 * 86_400u64), // 90 days
        &Address::generate(&env),
    );

    // Submit first claim
    env.ledger().set_timestamp(1000 + 10 * 86_400);
    client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &100_000_000i128,
        &String::from_str(&env, "First claim"),
    );

    // Try to submit second claim too soon (cooldown is 7 days)
    env.ledger().set_timestamp(1000 + 15 * 86_400); // Only 5 days later
    client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &100_000_000i128,
        &String::from_str(&env, "Second claim"),
    );
}

#[test]
fn test_claim_after_cooldown() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, _, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(90 * 86_400u64),
        &Address::generate(&env),
    );

    // Submit first claim
    env.ledger().set_timestamp(1000 + 10 * 86_400);
    let claim_id_1 = client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &100_000_000i128,
        &String::from_str(&env, "First claim"),
    );

    // Submit second claim after cooldown (7+ days)
    env.ledger().set_timestamp(1000 + 18 * 86_400); // 8 days later
    let claim_id_2 = client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &100_000_000i128,
        &String::from_str(&env, "Second claim"),
    );

    assert_eq!(claim_id_2, claim_id_1 + 1);
}

#[test]
#[should_panic(expected = "User is flagged for suspicious activity")]
fn test_flagged_user_cannot_claim() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, admin, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );

    // Admin flags user
    client.flag_user(&admin, &user, &String::from_str(&env, "Suspicious pattern"));

    // Try to submit claim
    env.ledger().set_timestamp(1000 + 10 * 86_400);
    client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &500_000_000i128,
        &String::from_str(&env, "Test"),
    );
}

#[test]
fn test_unflag_user() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, admin, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    // Flag user
    client.flag_user(&admin, &user, &String::from_str(&env, "Test flag"));

    let metrics = client.get_fraud_metrics(&user).unwrap();
    assert!(metrics.flagged);

    // Unflag user
    client.unflag_user(&admin, &user);

    let metrics = client.get_fraud_metrics(&user).unwrap();
    assert!(!metrics.flagged);

    // Now user should be able to purchase policy and claim
    payment_admin_client.mint(&user, &10_000_000_000i128);

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );

    env.ledger().set_timestamp(1000 + 10 * 86_400);
    client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &500_000_000i128,
        &String::from_str(&env, "Test"),
    );
}

// ───────────── PREMIUM POOL TESTS ─────────────

#[test]
fn test_add_to_pool() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&admin, &10_000_000_000i128);

    let initial_pool = client.get_premium_pool();

    client.add_to_pool(&admin, &5_000_000_000i128);

    let final_pool = client.get_premium_pool();
    assert_eq!(final_pool, initial_pool + 5_000_000_000);
}

#[test]
fn test_withdraw_from_pool() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, payment_token_client, payment_admin_client) =
        setup_insurance_contract(&env);

    payment_admin_client.mint(&admin, &10_000_000_000i128);
    client.add_to_pool(&admin, &5_000_000_000i128);

    let pool_before = client.get_premium_pool();
    let balance_before = payment_token_client.balance(&admin);

    client.withdraw_from_pool(&admin, &2_000_000_000i128);

    let pool_after = client.get_premium_pool();
    let balance_after = payment_token_client.balance(&admin);

    assert_eq!(pool_after, pool_before - 2_000_000_000);
    assert_eq!(balance_after, balance_before + 2_000_000_000);
}

#[test]
#[should_panic(expected = "Insufficient pool balance")]
fn test_cannot_withdraw_more_than_pool() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&admin, &10_000_000_000i128);
    client.add_to_pool(&admin, &1_000_000_000i128);

    client.withdraw_from_pool(&admin, &2_000_000_000i128);
}

// ───────────── ADMIN FUNCTION TESTS ─────────────

#[test]
fn test_update_premium_rates() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, _, _) = setup_insurance_contract(&env);

    client.update_premium_rates(&admin, &200u32, &200u32, &150u32, &250u32);

    let config = client.get_config();
    assert_eq!(config.base_premium_rate, 200);
    assert_eq!(config.nft_multiplier, 200);
    assert_eq!(config.token_multiplier, 150);
    assert_eq!(config.combined_multiplier, 250);
}

#[test]
fn test_update_coverage_limits() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, _, _) = setup_insurance_contract(&env);

    client.update_coverage_limits(
        &admin,
        &(14 * 86_400u64),
        &(730 * 86_400u64), // 2 years
        &10_000_000_000_000i128,
    );

    let config = client.get_config();
    assert_eq!(config.min_coverage_period, 14 * 86_400);
    assert_eq!(config.max_coverage_period, 730 * 86_400);
    assert_eq!(config.max_coverage_amount, 10_000_000_000_000);
}

#[test]
fn test_update_fraud_params() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, _, _) = setup_insurance_contract(&env);

    client.update_fraud_params(&admin, &5u32, &(14 * 86_400u64));

    let config = client.get_config();
    assert_eq!(config.max_claims_per_period, 5);
    assert_eq!(config.claim_cooldown, 14 * 86_400);
}

#[test]
fn test_pause_contract() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, _, _) = setup_insurance_contract(&env);

    client.set_paused(&admin, &true);

    let config = client.get_config();
    assert!(config.paused);

    client.set_paused(&admin, &false);

    let config = client.get_config();
    assert!(!config.paused);
}

#[test]
#[should_panic(expected = "Contract is paused")]
fn test_cannot_purchase_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, user, _, _, _) = setup_insurance_contract(&env);

    client.set_paused(&admin, &true);

    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );
}

#[test]
fn test_emergency_withdraw() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _, _, payment_token_client, payment_admin_client) =
        setup_insurance_contract(&env);

    payment_admin_client.mint(&admin, &10_000_000_000i128);
    client.add_to_pool(&admin, &5_000_000_000i128);

    let balance_before = payment_token_client.balance(&admin);
    let withdrawn = client.emergency_withdraw(&admin);

    assert_eq!(withdrawn, 5_000_000_000);
    assert_eq!(client.get_premium_pool(), 0);

    let balance_after = payment_token_client.balance(&admin);
    assert_eq!(balance_after, balance_before + withdrawn);
}

// ───────────── INTEGRATION TESTS ─────────────

#[test]
fn test_full_insurance_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, admin, user, _, payment_token_client, payment_admin_client) =
        setup_insurance_contract(&env);

    // 1. Admin adds funds to pool
    payment_admin_client.mint(&admin, &20_000_000_000i128);
    client.add_to_pool(&admin, &10_000_000_000i128);

    // 2. User purchases policy
    payment_admin_client.mint(&user, &10_000_000_000i128);
    let asset_addr = Address::generate(&env);

    client.purchase_policy(
        &user,
        &CoverageType::Combined,
        &2_000_000_000i128,
        &(60 * 86_400u64),
        &asset_addr.clone(),
    );

    assert!(client.is_policy_active(&user));

    // 3. Time passes, user submits claim
    env.ledger().set_timestamp(1000 + 30 * 86_400);

    let claim_id = client.submit_claim(
        &user,
        &AssetType::Token,
        &asset_addr,
        &1_500_000_000i128,
        &String::from_str(&env, "Platform hack - lost tokens"),
    );

    // 4. Admin reviews and approves claim
    client.review_claim(
        &admin,
        &claim_id,
        &true,
        &String::from_str(&env, "Verified loss, approved payout"),
        &1_400_000_000i128,
    );

    // 5. Admin processes payout
    let balance_before = payment_token_client.balance(&user);
    client.process_payout(&admin, &claim_id);

    let balance_after = payment_token_client.balance(&user);
    assert_eq!(balance_after, balance_before + 1_400_000_000);

    // 6. Verify final state
    let claim = client.get_claim(&claim_id).unwrap();
    assert_eq!(claim.status, ClaimStatus::Paid);
    assert!(claim.payout_time > 0);

    let user_claims = client.get_user_claims(&user);
    assert_eq!(user_claims.len(), 1);
}

#[test]
fn test_policy_renewal_and_claim() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (client, admin, user, _, _, payment_admin_client) = setup_insurance_contract(&env);

    payment_admin_client.mint(&admin, &20_000_000_000i128);
    client.add_to_pool(&admin, &10_000_000_000i128);

    payment_admin_client.mint(&user, &10_000_000_000i128);

    // Purchase policy
    client.purchase_policy(
        &user,
        &CoverageType::Token,
        &1_000_000_000i128,
        &(30 * 86_400u64),
        &Address::generate(&env),
    );

    // Renew before expiration
    env.ledger().set_timestamp(1000 + 25 * 86_400);
    client.renew_policy(&user, &(30 * 86_400u64));

    // Submit claim in extended period
    env.ledger().set_timestamp(1000 + 40 * 86_400);
    let claim_id = client.submit_claim(
        &user,
        &AssetType::Token,
        &Address::generate(&env),
        &800_000_000i128,
        &String::from_str(&env, "Loss during extended coverage"),
    );

    assert!(claim_id > 0);
}
