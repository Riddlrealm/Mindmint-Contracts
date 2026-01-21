#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String,
};

fn create_token_contract<'a>(e: &'a Env, admin: &Address) -> (Address, TokenClient<'a>) {
    let sac = e.register_stellar_asset_contract_v2(admin.clone());
    let address = sac.address();
    (address.clone(), TokenClient::new(e, &address))
}

fn create_referral_contract(e: &Env) -> Address {
    e.register_contract(None, ReferralContract)
}

fn setup_contract(e: &Env) -> (Address, Address, Address, TokenClient) {
    e.mock_all_auths();
    
    let admin = Address::generate(e);
    let token_admin = Address::generate(e);
    let (token_address, token_client) = create_token_contract(e, &token_admin);
    let referral_contract = create_referral_contract(e);
    
    let token_admin_client = StellarAssetClient::new(e, &token_address);

    let client = ReferralContractClient::new(e, &referral_contract);
    client.initialize(
        &admin,
        &token_address,
        &1000,  // referrer reward
        &500,   // referee reward
        &10,    // max referrals per user
    );

    // Mint tokens to referral contract for rewards
    token_admin_client.mint(&referral_contract, &1_000_000);

    (referral_contract, token_address, admin, token_client)
}

#[test]
fn test_initialize() {
    let e = Env::default();
    e.mock_all_auths();
    
    let admin = Address::generate(&e);
    let token_admin = Address::generate(&e);
    let (token_address, _) = create_token_contract(&e, &token_admin);
    let referral_contract = create_referral_contract(&e);

    let client = ReferralContractClient::new(&e, &referral_contract);
    client.initialize(
        &admin,
        &token_address,
        &1000,
        &500,
        &10,
    );

    let config = client.get_config();
    assert_eq!(config.referrer_reward, 1000);
    assert_eq!(config.referee_reward, 500);
    assert_eq!(config.max_referrals_per_user, 10);

    let stats = client.get_statistics();
    assert_eq!(stats.total_referrals, 0);
    assert_eq!(stats.total_rewarded_referrals, 0);
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_initialize_twice_should_fail() {
    let e = Env::default();
    e.mock_all_auths();
    
    let admin = Address::generate(&e);
    let token_admin = Address::generate(&e);
    let (token_address, _) = create_token_contract(&e, &token_admin);
    let referral_contract = create_referral_contract(&e);

    let client = ReferralContractClient::new(&e, &referral_contract);
    client.initialize(&admin, &token_address, &1000, &500, &10);
    client.initialize(&admin, &token_address, &1000, &500, &10);
}

#[test]
fn test_generate_referral_code() {
    let e = Env::default();
    let (referral_contract, _, _, _) = setup_contract(&e);
    let user = Address::generate(&e);

    let client = ReferralContractClient::new(&e, &referral_contract);
    let code = client.generate_referral_code(&user);

    assert!(!code.is_empty());
    
    // Verify code can be retrieved
    let retrieved_code = client.get_referral_code(&user);
    assert_eq!(retrieved_code, Some(code.clone()));

    // Verify code owner lookup
    let owner = client.get_code_owner(&code);
    assert_eq!(owner, Some(user.clone()));

    // Verify initial referral count is 0
    assert_eq!(client.get_referral_count(&user), 0);
}

#[test]
#[should_panic(expected = "Referral code already exists")]
fn test_generate_referral_code_twice_should_fail() {
    let e = Env::default();
    let (referral_contract, _, _, _) = setup_contract(&e);
    let user = Address::generate(&e);

    let client = ReferralContractClient::new(&e, &referral_contract);
    client.generate_referral_code(&user);
    client.generate_referral_code(&user);
}

#[test]
fn test_register_with_referral_code() {
    let e = Env::default();
    let (referral_contract, token_address, _, token_client) = setup_contract(&e);
    
    let referrer = Address::generate(&e);
    let referee = Address::generate(&e);

    let client = ReferralContractClient::new(&e, &referral_contract);
    let code = client.generate_referral_code(&referrer);

    // Register referee
    let result = client.register_with_referral_code(&referee, &code);
    assert!(result);

    // Verify referral relationship
    let retrieved_referrer = client.get_referrer(&referee);
    assert_eq!(retrieved_referrer, Some(referrer.clone()));

    // Verify referral count increased
    assert_eq!(client.get_referral_count(&referrer), 1);

    // Verify referee is in referrer's list
    let referrals = client.get_referrals(&referrer);
    assert_eq!(referrals.len(), 1);
    assert_eq!(referrals.get(0), Some(referee.clone()));

    // Verify statistics
    let stats = client.get_statistics();
    assert_eq!(stats.total_referrals, 1);
    assert_eq!(stats.total_rewarded_referrals, 1);
    assert_eq!(stats.total_referrer_rewards, 1000);
    assert_eq!(stats.total_referee_rewards, 500);

    // Verify token balances (if rewards were distributed)
    assert_eq!(token_client.balance(&referrer), 1000);
    assert_eq!(token_client.balance(&referee), 500);
}

#[test]
#[should_panic(expected = "Invalid referral code")]
fn test_register_with_invalid_code_should_fail() {
    let e = Env::default();
    let (referral_contract, _, _, _) = setup_contract(&e);
    let referee = Address::generate(&e);

    let client = ReferralContractClient::new(&e, &referral_contract);
    let invalid_code = String::from_str(&e, "INVALID");
    
    client.register_with_referral_code(&referee, &invalid_code);
}

#[test]
#[should_panic(expected = "Cannot refer yourself")]
fn test_self_referral_should_fail() {
    let e = Env::default();
    let (referral_contract, _, _, _) = setup_contract(&e);
    let user = Address::generate(&e);

    let client = ReferralContractClient::new(&e, &referral_contract);
    let code = client.generate_referral_code(&user);
    
    // Try to refer yourself
    client.register_with_referral_code(&user, &code);
}

#[test]
#[should_panic(expected = "Already registered with a referral code")]
fn test_duplicate_registration_should_fail() {
    let e = Env::default();
    let (referral_contract, _, _, _) = setup_contract(&e);
    
    let referrer1 = Address::generate(&e);
    let referrer2 = Address::generate(&e);
    let referee = Address::generate(&e);

    let client = ReferralContractClient::new(&e, &referral_contract);
    let code1 = client.generate_referral_code(&referrer1);
    let code2 = client.generate_referral_code(&referrer2);

    // First registration
    client.register_with_referral_code(&referee, &code1);

    // Try to register again with different code
    client.register_with_referral_code(&referee, &code2);
}

#[test]
#[should_panic(expected = "Referrer has reached maximum referral limit")]
fn test_referral_limit() {
    let e = Env::default();
    let (referral_contract, _, _, _) = setup_contract(&e);
    
    let referrer = Address::generate(&e);
    let client = ReferralContractClient::new(&e, &referral_contract);
    let code = client.generate_referral_code(&referrer);

    // Register max referrals
    for i in 0..10 {
        let referee = Address::generate(&e);
        client.register_with_referral_code(&referee, &code);
        assert_eq!(client.get_referral_count(&referrer), (i + 1) as u32);
    }

    // Try to register one more (should fail)
    let extra_referee = Address::generate(&e);
    client.register_with_referral_code(&extra_referee, &code);
}

#[test]
fn test_multiple_referrals() {
    let e = Env::default();
    let (referral_contract, _, _, _) = setup_contract(&e);
    
    let referrer = Address::generate(&e);
    let client = ReferralContractClient::new(&e, &referral_contract);
    let code = client.generate_referral_code(&referrer);

    let referee1 = Address::generate(&e);
    let referee2 = Address::generate(&e);
    let referee3 = Address::generate(&e);

    client.register_with_referral_code(&referee1, &code);
    client.register_with_referral_code(&referee2, &code);
    client.register_with_referral_code(&referee3, &code);

    assert_eq!(client.get_referral_count(&referrer), 3);
    
    let referrals = client.get_referrals(&referrer);
    assert_eq!(referrals.len(), 3);
}

#[test]
fn test_statistics_tracking() {
    let e = Env::default();
    let (referral_contract, _, _, _) = setup_contract(&e);
    
    let referrer1 = Address::generate(&e);
    let referrer2 = Address::generate(&e);
    let client = ReferralContractClient::new(&e, &referral_contract);
    
    let code1 = client.generate_referral_code(&referrer1);
    let code2 = client.generate_referral_code(&referrer2);

    // First referral
    let referee1 = Address::generate(&e);
    client.register_with_referral_code(&referee1, &code1);

    let stats = client.get_statistics();
    assert_eq!(stats.total_referrals, 1);
    assert_eq!(stats.total_rewarded_referrals, 1);

    // Second referral
    let referee2 = Address::generate(&e);
    client.register_with_referral_code(&referee2, &code2);

    let stats = client.get_statistics();
    assert_eq!(stats.total_referrals, 2);
    assert_eq!(stats.total_rewarded_referrals, 2);
    assert_eq!(stats.total_referrer_rewards, 2000);
    assert_eq!(stats.total_referee_rewards, 1000);

    // Third referral
    let referee3 = Address::generate(&e);
    client.register_with_referral_code(&referee3, &code1);

    let stats = client.get_statistics();
    assert_eq!(stats.total_referrals, 3);
    assert_eq!(stats.total_referrer_rewards, 3000);
    assert_eq!(stats.total_referee_rewards, 1500);
}

#[test]
fn test_update_config() {
    let e = Env::default();
    let (referral_contract, _, admin, _) = setup_contract(&e);

    let client = ReferralContractClient::new(&e, &referral_contract);
    
    // Update config
    client.update_config(
        &admin,
        &Some(2000),  // new referrer reward
        &Some(1000),  // new referee reward
        &Some(20),    // new max referrals
    );

    let config = client.get_config();
    assert_eq!(config.referrer_reward, 2000);
    assert_eq!(config.referee_reward, 1000);
    assert_eq!(config.max_referrals_per_user, 20);
}

#[test]
#[should_panic(expected = "Admin only")]
fn test_update_config_non_admin_should_fail() {
    let e = Env::default();
    let (referral_contract, _, _, _) = setup_contract(&e);
    let non_admin = Address::generate(&e);

    let client = ReferralContractClient::new(&e, &referral_contract);
    client.update_config(&non_admin, &Some(2000), &None, &None);
}

#[test]
fn test_unique_referral_codes() {
    let e = Env::default();
    let (referral_contract, _, _, _) = setup_contract(&e);

    let client = ReferralContractClient::new(&e, &referral_contract);
    
    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);
    let user3 = Address::generate(&e);

    let code1 = client.generate_referral_code(&user1);
    let code2 = client.generate_referral_code(&user2);
    let code3 = client.generate_referral_code(&user3);

    // Verify all codes are unique
    assert_ne!(code1, code2);
    assert_ne!(code1, code3);
    assert_ne!(code2, code3);

    // Verify each code maps to correct owner
    assert_eq!(client.get_code_owner(&code1), Some(user1));
    assert_eq!(client.get_code_owner(&code2), Some(user2));
    assert_eq!(client.get_code_owner(&code3), Some(user3));
}

#[test]
fn test_referral_chain() {
    let e = Env::default();
    let (referral_contract, _, _, _) = setup_contract(&e);

    let client = ReferralContractClient::new(&e, &referral_contract);
    
    // Create a referral chain: A refers B, B refers C
    let user_a = Address::generate(&e);
    let user_b = Address::generate(&e);
    let user_c = Address::generate(&e);

    let code_a = client.generate_referral_code(&user_a);
    let code_b = client.generate_referral_code(&user_b);

    // A refers B
    client.register_with_referral_code(&user_b, &code_a);
    assert_eq!(client.get_referrer(&user_b), Some(user_a.clone()));

    // B refers C
    client.register_with_referral_code(&user_c, &code_b);
    assert_eq!(client.get_referrer(&user_c), Some(user_b.clone()));

    // Verify counts
    assert_eq!(client.get_referral_count(&user_a), 1);
    assert_eq!(client.get_referral_count(&user_b), 1);
    assert_eq!(client.get_referral_count(&user_c), 0);
}

#[test]
fn test_get_referrals_list() {
    let e = Env::default();
    let (referral_contract, _, _, _) = setup_contract(&e);

    let referrer = Address::generate(&e);
    let client = ReferralContractClient::new(&e, &referral_contract);
    let code = client.generate_referral_code(&referrer);

    let referee1 = Address::generate(&e);
    let referee2 = Address::generate(&e);
    let referee3 = Address::generate(&e);

    client.register_with_referral_code(&referee1, &code);
    client.register_with_referral_code(&referee2, &code);
    client.register_with_referral_code(&referee3, &code);

    let referrals = client.get_referrals(&referrer);
    assert_eq!(referrals.len(), 3);
    assert_eq!(referrals.get(0), Some(referee1));
    assert_eq!(referrals.get(1), Some(referee2));
    assert_eq!(referrals.get(2), Some(referee3));
}
