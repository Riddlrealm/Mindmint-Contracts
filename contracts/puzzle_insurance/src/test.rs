#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::Client as TokenClient,
    token::StellarAssetClient,
    Address, Env,
};

fn create_token<'a>(
    env: &'a Env,
    admin: &Address,
) -> (Address, TokenClient<'a>, StellarAssetClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let address = sac.address();
    (
        address.clone(),
        TokenClient::new(env, &address),
        StellarAssetClient::new(env, &address),
    )
}

fn setup<'a>(
    env: &'a Env,
) -> (
    Address,
    Address,
    Address,
    Address,
    PuzzleInsuranceContractClient<'a>,
    TokenClient<'a>,
    StellarAssetClient<'a>,
) {
    env.mock_all_auths();

    let token_admin = Address::generate(env);
    let (payment_token, token_client, token_admin_client) = create_token(env, &token_admin);

    let admin = Address::generate(env);
    let user = Address::generate(env);

    let contract_id = env.register_contract(None, PuzzleInsuranceContract);
    let client = PuzzleInsuranceContractClient::new(env, &contract_id);

    client.initialize(&admin, &payment_token, &1000i128);

    let huge = 1_000_000_000_000_000i128;
    token_admin_client.mint(&user, &huge);
    token_admin_client.mint(&contract_id, &huge);

    (
        admin,
        payment_token,
        user,
        token_admin,
        client,
        token_client,
        token_admin_client,
    )
}

#[test]
fn test_initialize() {
    let env = Env::default();
    let (admin, payment_token, _user, _tok_adm, client, _tc, _tac) = setup(&env);

    let config = client.get_config();
    assert_eq!(config.admin, admin);
    assert_eq!(config.payment_token, payment_token);
    assert_eq!(config.base_rate, 1000i128);
    assert_eq!(config.max_coverage_percent, 8000);
}

#[test]
fn test_purchase_policy() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);

    let policy_id = client.purchase_policy(&user, &5, &86400, &5000);
    assert_eq!(policy_id, 1);

    let policy = client.get_policy(&policy_id).unwrap();
    assert_eq!(policy.holder, user);
    assert_eq!(policy.attempts_covered, 5);
    assert_eq!(policy.attempts_used, 0);
    assert_eq!(policy.coverage_percent, 5000);
    assert_eq!(policy.premium_paid, 2500i128);
    assert!(policy.active);
}

#[test]
#[should_panic(expected = "Coverage percent exceeds maximum")]
fn test_purchase_policy_exceeds_max_coverage() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);
    client.purchase_policy(&user, &5, &86400, &9000);
}

#[test]
#[should_panic(expected = "Invalid attempts count")]
fn test_purchase_policy_invalid_attempts() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);
    client.purchase_policy(&user, &0, &86400, &5000);
}

#[test]
fn test_file_claim_valid() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);

    let policy_id = client.purchase_policy(&user, &3, &86400, &5000);

    let payout = client.file_claim(&policy_id, &1000i128);
    assert_eq!(payout, 500i128);

    let policy = client.get_policy(&policy_id).unwrap();
    assert_eq!(policy.attempts_used, 1);
    assert!(policy.active);
}

#[test]
#[should_panic(expected = "Policy is not active")]
fn test_file_claim_inactive_policy() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);

    let policy_id = client.purchase_policy(&user, &1, &86400, &5000);
    client.expire_policy(&policy_id);
    client.file_claim(&policy_id, &1000i128);
}

#[test]
#[should_panic(expected = "Policy is not active")]
fn test_file_claim_exhausted_attempts() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);

    let policy_id = client.purchase_policy(&user, &2, &86400, &5000);

    client.file_claim(&policy_id, &1000i128);
    client.file_claim(&policy_id, &1000i128);

    client.file_claim(&policy_id, &1000i128);
}

#[test]
fn test_policy_expires_by_time() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);

    let policy_id = client.purchase_policy(&user, &3, &86400, &5000);

    env.ledger().set_timestamp(env.ledger().timestamp() + 86401);

    let policy = client.get_policy(&policy_id).unwrap();
    assert!(!policy.active);
}

#[test]
fn test_policy_expires_by_attempts() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);

    let policy_id = client.purchase_policy(&user, &2, &86400, &5000);

    client.file_claim(&policy_id, &1000i128);
    client.file_claim(&policy_id, &1000i128);

    let policy = client.get_policy(&policy_id).unwrap();
    assert!(!policy.active);
}

#[test]
fn test_set_base_rate() {
    let env = Env::default();
    let (admin, _pt, _u, _ta, client, _tc, _tac) = setup(&env);

    client.set_base_rate(&admin, &2000i128);

    let config = client.get_config();
    assert_eq!(config.base_rate, 2000i128);
}

#[test]
#[should_panic(expected = "Not admin")]
fn test_set_base_rate_unauthorized() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);
    client.set_base_rate(&user, &2000i128);
}

#[test]
fn test_set_max_coverage_percent() {
    let env = Env::default();
    let (admin, _pt, _u, _ta, client, _tc, _tac) = setup(&env);

    client.set_max_coverage_percent(&admin, &9000);

    let config = client.get_config();
    assert_eq!(config.max_coverage_percent, 9000);
}

#[test]
#[should_panic(expected = "Invalid max coverage percent")]
fn test_set_max_coverage_percent_invalid() {
    let env = Env::default();
    let (admin, _pt, _u, _ta, client, _tc, _tac) = setup(&env);
    client.set_max_coverage_percent(&admin, &11000);
}

#[test]
fn test_get_user_policies() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);

    let policy1 = client.purchase_policy(&user, &3, &86400, &5000);
    let policy2 = client.purchase_policy(&user, &2, &86400, &7000);

    let user_policies = client.get_user_policies(&user);
    assert_eq!(user_policies.len(), 2);
    assert!(user_policies.contains(&policy1));
    assert!(user_policies.contains(&policy2));
}

#[test]
fn test_premium_calculation() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);

    let policy1 = client.purchase_policy(&user, &1, &86400, &2500);
    assert_eq!(policy1, 1);
    let policy1_data = client.get_policy(&policy1).unwrap();
    assert_eq!(policy1_data.premium_paid, 250i128);

    let policy2 = client.purchase_policy(&user, &2, &86400, &7500);
    assert_eq!(policy2, 2);
    let policy2_data = client.get_policy(&policy2).unwrap();
    assert_eq!(policy2_data.premium_paid, 1500i128);
}

#[test]
#[should_panic(expected = "Loss amount must be positive")]
fn test_file_claim_invalid_amount() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);

    let policy_id = client.purchase_policy(&user, &3, &86400, &5000);
    client.file_claim(&policy_id, &-100i128);
}

#[test]
#[should_panic(expected = "Policy not found")]
fn test_file_claim_nonexistent_policy() {
    let env = Env::default();
    let (_a, _pt, _u, _ta, client, _tc, _tac) = setup(&env);
    client.file_claim(&999, &1000i128);
}

#[test]
fn test_manual_policy_expiry() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);

    let policy_id = client.purchase_policy(&user, &3, &86400, &5000);
    client.expire_policy(&policy_id);

    let policy = client.get_policy(&policy_id).unwrap();
    assert!(!policy.active);
}

#[test]
#[should_panic(expected = "Policy already inactive")]
fn test_expire_already_inactive_policy() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);

    let policy_id = client.purchase_policy(&user, &1, &86400, &5000);
    client.expire_policy(&policy_id);
    client.expire_policy(&policy_id);
}

#[test]
fn test_payout_calculation() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);

    let policy_id = client.purchase_policy(&user, &3, &86400, &3000);

    let payout1 = client.file_claim(&policy_id, &10000i128);
    assert_eq!(payout1, 3000i128);

    let payout2 = client.file_claim(&policy_id, &5000i128);
    assert_eq!(payout2, 1500i128);
}

#[test]
#[should_panic(expected = "Base rate must be positive")]
fn test_set_base_rate_invalid() {
    let env = Env::default();
    let (admin, _pt, _u, _ta, client, _tc, _tac) = setup(&env);
    client.set_base_rate(&admin, &0i128);
}

#[test]
fn test_maximum_duration() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);

    let max_duration = 365 * 24 * 60 * 60;
    let policy_id = client.purchase_policy(&user, &1, &max_duration, &5000);

    let policy = client.get_policy(&policy_id).unwrap();
    assert!(policy.active);
    assert_eq!(policy.expires_at, env.ledger().timestamp() + max_duration);
}

#[test]
#[should_panic(expected = "Invalid duration")]
fn test_duration_too_long() {
    let env = Env::default();
    let (_a, _pt, user, _ta, client, _tc, _tac) = setup(&env);
    let too_long = 366 * 24 * 60 * 60;
    client.purchase_policy(&user, &1, &too_long, &5000);
}
