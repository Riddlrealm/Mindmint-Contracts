#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

fn setup_token<'a>(
    env: &'a Env,
    admin: &'a Address,
) -> (Address, token::Client<'a>, token::StellarAssetClient<'a>) {
    let token_id = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_client = token::Client::new(env, &token_id);
    let token_admin_client = token::StellarAssetClient::new(env, &token_id);
    (token_id, token_client, token_admin_client)
}

#[test]
fn test_token_loan_partial_repayment() {
    let env = Env::default();
    env.mock_all_auths();

    let lender = Address::generate(&env);
    let borrower = Address::generate(&env);
    let loan_admin = Address::generate(&env);
    let collateral_admin = Address::generate(&env);

    let (loan_token, loan_client, loan_admin_client) = setup_token(&env, &loan_admin);
    let (collateral_token, collateral_client, collateral_admin_client) =
        setup_token(&env, &collateral_admin);

    loan_admin_client.mint(&lender, &2_000);
    collateral_admin_client.mint(&borrower, &5_000);

    let contract_id = env.register_contract(None, LendingContract);
    let client = LendingContractClient::new(&env, &contract_id);

    let offer_id = client.create_offer(
        &lender,
        &Asset {
            asset_type: AssetType::Token,
            contract: loan_token.clone(),
            amount: 1_000,
            nft_id: 0,
        },
        &Asset {
            asset_type: AssetType::Token,
            contract: collateral_token.clone(),
            amount: 1_500,
            nft_id: 0,
        },
        &loan_token,
        &1_000,
        &LoanTerms {
            duration_secs: 1_000,
            interest_bps: 1_000,
            max_extension_secs: 500,
        },
    );

    let loan_id = client.accept_offer(&borrower, &offer_id);

    assert_eq!(loan_client.balance(&borrower), 1_000);
    assert_eq!(collateral_client.balance(&borrower), 3_500);

    env.ledger().set_timestamp(500);
    client.repay(&borrower, &loan_id, &500);

    let loan = client.get_loan(&loan_id).unwrap();
    assert_eq!(loan.outstanding_principal, 550);
    assert_eq!(loan.accrued_interest, 0);

    loan_admin_client.mint(&borrower, &100);
    env.ledger().set_timestamp(1_000);
    client.repay(&borrower, &loan_id, &577);

    let loan = client.get_loan(&loan_id).unwrap();
    assert_eq!(loan.status, LoanStatus::Repaid);
    assert_eq!(loan.outstanding_principal, 0);
    assert_eq!(loan.accrued_interest, 0);

    assert_eq!(collateral_client.balance(&borrower), 5_000);
    assert_eq!(loan_client.balance(&lender), 2_077);
}

#[test]
fn test_liquidation_on_default() {
    let env = Env::default();
    env.mock_all_auths();

    let lender = Address::generate(&env);
    let borrower = Address::generate(&env);
    let loan_admin = Address::generate(&env);
    let collateral_admin = Address::generate(&env);

    let (loan_token, _loan_client, loan_admin_client) = setup_token(&env, &loan_admin);
    let (collateral_token, collateral_client, collateral_admin_client) =
        setup_token(&env, &collateral_admin);

    loan_admin_client.mint(&lender, &1_000);
    collateral_admin_client.mint(&borrower, &2_000);

    let contract_id = env.register_contract(None, LendingContract);
    let client = LendingContractClient::new(&env, &contract_id);

    let offer_id = client.create_offer(
        &lender,
        &Asset {
            asset_type: AssetType::Token,
            contract: loan_token.clone(),
            amount: 1_000,
            nft_id: 0,
        },
        &Asset {
            asset_type: AssetType::Token,
            contract: collateral_token.clone(),
            amount: 1_500,
            nft_id: 0,
        },
        &loan_token,
        &1_000,
        &LoanTerms {
            duration_secs: 100,
            interest_bps: 1_000,
            max_extension_secs: 0,
        },
    );

    let loan_id = client.accept_offer(&borrower, &offer_id);

    env.ledger().set_timestamp(200);
    client.liquidate(&lender, &loan_id);

    let loan = client.get_loan(&loan_id).unwrap();
    assert_eq!(loan.status, LoanStatus::Liquidated);
    assert_eq!(collateral_client.balance(&lender), 1_500);
}

#[test]
fn test_extension_request_and_approval() {
    let env = Env::default();
    env.mock_all_auths();

    let lender = Address::generate(&env);
    let borrower = Address::generate(&env);
    let loan_admin = Address::generate(&env);
    let collateral_admin = Address::generate(&env);

    let (loan_token, _loan_client, loan_admin_client) = setup_token(&env, &loan_admin);
    let (collateral_token, _collateral_client, collateral_admin_client) =
        setup_token(&env, &collateral_admin);

    loan_admin_client.mint(&lender, &1_000);
    collateral_admin_client.mint(&borrower, &2_000);

    let contract_id = env.register_contract(None, LendingContract);
    let client = LendingContractClient::new(&env, &contract_id);

    let offer_id = client.create_offer(
        &lender,
        &Asset {
            asset_type: AssetType::Token,
            contract: loan_token.clone(),
            amount: 1_000,
            nft_id: 0,
        },
        &Asset {
            asset_type: AssetType::Token,
            contract: collateral_token.clone(),
            amount: 1_500,
            nft_id: 0,
        },
        &loan_token,
        &1_000,
        &LoanTerms {
            duration_secs: 100,
            interest_bps: 500,
            max_extension_secs: 200,
        },
    );

    let loan_id = client.accept_offer(&borrower, &offer_id);

    client.request_extension(&borrower, &loan_id, &150);
    let request = client.get_extension_request(&loan_id).unwrap();
    assert_eq!(request.requested_extension_secs, 150);

    client.approve_extension(&lender, &loan_id);
    let loan = client.get_loan(&loan_id).unwrap();
    assert_eq!(loan.due_time, 250);
    assert_eq!(client.get_extension_request(&loan_id), None);
}

#[test]
fn test_nft_loan_repayment_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let lender = Address::generate(&env);
    let borrower = Address::generate(&env);
    let loan_admin = Address::generate(&env);
    let collateral_admin = Address::generate(&env);

    let (repayment_token, repayment_client, repayment_admin_client) =
        setup_token(&env, &loan_admin);
    let (collateral_token, collateral_client, collateral_admin_client) =
        setup_token(&env, &collateral_admin);

    repayment_admin_client.mint(&borrower, &500);
    collateral_admin_client.mint(&borrower, &2_000);

    let nft_contract_id = env.register_contract(None, MockNft);
    let nft_client = MockNftClient::new(&env, &nft_contract_id);
    let nft_id: u32 = 1;
    nft_client.mint(&lender, &nft_id);

    let contract_id = env.register_contract(None, LendingContract);
    let client = LendingContractClient::new(&env, &contract_id);

    let offer_id = client.create_offer(
        &lender,
        &Asset {
            asset_type: AssetType::NFT,
            contract: nft_contract_id.clone(),
            amount: 1,
            nft_id,
        },
        &Asset {
            asset_type: AssetType::Token,
            contract: collateral_token.clone(),
            amount: 1_000,
            nft_id: 0,
        },
        &repayment_token,
        &300,
        &LoanTerms {
            duration_secs: 100,
            interest_bps: 1_000,
            max_extension_secs: 0,
        },
    );

    let loan_id = client.accept_offer(&borrower, &offer_id);
    assert_eq!(nft_client.owner_of(&nft_id), borrower.clone());

    env.ledger().set_timestamp(100);
    client.repay(&borrower, &loan_id, &330);

    let loan = client.get_loan(&loan_id).unwrap();
    assert_eq!(loan.status, LoanStatus::Repaid);
    assert_eq!(nft_client.owner_of(&nft_id), lender.clone());
    assert_eq!(collateral_client.balance(&borrower), 2_000);
    assert_eq!(repayment_client.balance(&lender), 330);
}

#[contracttype]
enum MockNftDataKey {
    Owner(u32),
}

#[contract]
struct MockNft;

#[contractimpl]
impl MockNft {
    pub fn mint(env: Env, to: Address, token_id: u32) {
        env.storage()
            .persistent()
            .set(&MockNftDataKey::Owner(token_id), &to);
    }

    pub fn transfer(env: Env, from: Address, to: Address, token_id: u32) {
        from.require_auth();
        let owner: Address = env
            .storage()
            .persistent()
            .get(&MockNftDataKey::Owner(token_id))
            .unwrap();
        if owner != from {
            panic!("Not owner");
        }
        env.storage()
            .persistent()
            .set(&MockNftDataKey::Owner(token_id), &to);
    }

    pub fn owner_of(env: Env, token_id: u32) -> Address {
        env.storage()
            .persistent()
            .get(&MockNftDataKey::Owner(token_id))
            .unwrap()
    }
}
