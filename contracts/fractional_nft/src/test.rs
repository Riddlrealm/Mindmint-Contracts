#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

// ---------------------------------------------------------------------------
// Mock NFT contract
// ---------------------------------------------------------------------------

#[contract]
pub struct MockNFT;

#[contractimpl]
impl MockNFT {
    pub fn owner_of(env: Env, token_id: u32) -> Address {
        env.storage()
            .persistent()
            .get(&token_id)
            .unwrap_or_else(|| panic!("token_not_found"))
    }

    pub fn transfer(env: Env, from: Address, to: Address, token_id: u32) {
        from.require_auth();
        let current_owner: Address = env
            .storage()
            .persistent()
            .get(&token_id)
            .unwrap_or_else(|| panic!("token_not_found"));
        if current_owner != from {
            panic!("not_owner");
        }
        env.storage().persistent().set(&token_id, &to);
    }

    pub fn set_owner(env: Env, token_id: u32, owner: Address) {
        env.storage().persistent().set(&token_id, &owner);
    }
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn setup_token<'a>(
    env: &'a Env,
    admin: &'a Address,
) -> (Address, token::Client<'a>, token::StellarAssetClient<'a>) {
    let token_id = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let client = token::Client::new(env, &token_id);
    let admin_client = token::StellarAssetClient::new(env, &token_id);
    (token_id, client, admin_client)
}

fn setup_nft(env: &Env) -> (Address, MockNFTClient) {
    let nft_addr = env.register_contract(None, MockNFT);
    let client = MockNFTClient::new(env, &nft_addr);
    (nft_addr, client)
}

fn setup_vault(env: &Env) -> (Address, FractionalNftContractClient) {
    let addr = env.register_contract(None, FractionalNftContract);
    let client = FractionalNftContractClient::new(env, &addr);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (addr, client)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// fractionalize locks the NFT and credits all fractions to the owner.
#[test]
fn test_fractionalize_basic() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let (payment_token_id, _, _) = setup_token(&env, &admin);
    let (nft_addr, nft) = setup_nft(&env);
    nft.set_owner(&1u32, &owner);

    let (vault_addr, vault) = setup_vault(&env);

    let vid = vault.fractionalize(
        &owner,
        &nft_addr,
        &1u32,
        &1_000i128,
        &10_000i128,
        &payment_token_id,
    );

    let state = vault.get_vault(&vid).unwrap();
    assert_eq!(state.total_fractions, 1_000);
    assert_eq!(state.buyout_price, 10_000);
    assert_eq!(state.status, VaultStatus::Active);
    assert_eq!(state.fraction_token, vault_addr);

    // All fractions credited to owner.
    assert_eq!(vault.balance_of(&vid, &owner), 1_000);

    // NFT is now held by the vault contract.
    assert_eq!(nft.owner_of(&1u32), vault_addr);
}

/// buy_fraction transfers fractions from owner to buyer, charging the correct
/// pro-rata price.
#[test]
fn test_buy_fraction() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (payment_token_id, payment_token, payment_admin) = setup_token(&env, &admin);
    payment_admin.mint(&buyer, &5_000i128);

    let (nft_addr, nft) = setup_nft(&env);
    nft.set_owner(&1u32, &owner);

    let (_, vault) = setup_vault(&env);

    // 1000 fractions, buyout_price = 10_000  ⟹  price_per_fraction = 10
    let vid = vault.fractionalize(
        &owner,
        &nft_addr,
        &1u32,
        &1_000i128,
        &10_000i128,
        &payment_token_id,
    );

    // Buyer purchases 100 fractions at 10 tokens each = 1_000 tokens.
    vault.buy_fraction(&buyer, &vid, &100i128);

    assert_eq!(vault.balance_of(&vid, &buyer), 100);
    assert_eq!(vault.balance_of(&vid, &owner), 900);
    // Owner received 1_000 tokens.
    assert_eq!(payment_token.balance(&owner), 1_000);
    // Buyer spent 1_000 tokens.
    assert_eq!(payment_token.balance(&buyer), 4_000);
}

/// transfer_fraction moves fractions between holders without payment.
#[test]
fn test_transfer_fraction() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let alice = Address::generate(&env);

    let (payment_token_id, _, _) = setup_token(&env, &admin);
    let (nft_addr, nft) = setup_nft(&env);
    nft.set_owner(&1u32, &owner);

    let (_, vault) = setup_vault(&env);
    let vid = vault.fractionalize(
        &owner,
        &nft_addr,
        &1u32,
        &100i128,
        &1_000i128,
        &payment_token_id,
    );

    vault.transfer_fraction(&owner, &vid, &alice, &30i128);

    assert_eq!(vault.balance_of(&vid, &owner), 70);
    assert_eq!(vault.balance_of(&vid, &alice), 30);
}

/// initiate_buyout requires offer_price >= buyout_price.
#[test]
#[should_panic(expected = "offer_below_buyout_price")]
fn test_initiate_buyout_low_offer() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (payment_token_id, _, payment_admin) = setup_token(&env, &admin);
    payment_admin.mint(&buyer, &5_000i128);

    let (nft_addr, nft) = setup_nft(&env);
    nft.set_owner(&1u32, &owner);

    let (_, vault) = setup_vault(&env);
    let vid = vault.fractionalize(
        &owner,
        &nft_addr,
        &1u32,
        &1_000i128,
        &10_000i128,
        &payment_token_id,
    );

    // Offer 5_000 but buyout_price is 10_000 → should panic.
    vault.initiate_buyout(&buyer, &vid, &5_000i128);
}

/// Full happy-path: buyout passes, NFT transfers, holders claim proceeds.
#[test]
fn test_buyout_passes_and_proceeds_claimed() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let holder = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (payment_token_id, payment_token, payment_admin) = setup_token(&env, &admin);
    payment_admin.mint(&buyer, &20_000i128);

    let (nft_addr, nft) = setup_nft(&env);
    nft.set_owner(&1u32, &owner);

    let (vault_addr, vault) = setup_vault(&env);

    // 1_000 fractions, buyout_price = 10_000.
    let vid = vault.fractionalize(
        &owner,
        &nft_addr,
        &1u32,
        &1_000i128,
        &10_000i128,
        &payment_token_id,
    );

    // Give holder 400 fractions (owner keeps 600).
    vault.transfer_fraction(&owner, &vid, &holder, &400i128);

    assert_eq!(vault.balance_of(&vid, &owner), 600);
    assert_eq!(vault.balance_of(&vid, &holder), 400);

    // Buyer initiates buyout at 12_000 (≥ 10_000).
    env.ledger().set_timestamp(1_000);
    vault.initiate_buyout(&buyer, &vid, &12_000i128);

    let state = vault.get_vault(&vid).unwrap();
    assert_eq!(state.status, VaultStatus::BuyoutPending);

    // Both owner (600) and holder (400) vote in favour → 1_000 / 1_000 = 100 %.
    vault.vote_buyout(&owner, &vid, &true);
    vault.vote_buyout(&holder, &vid, &true);

    let state = vault.get_vault(&vid).unwrap();
    assert_eq!(state.buyout_votes_for, 1_000);
    assert_eq!(state.buyout_votes_against, 0);

    // Advance past deadline.
    env.ledger().set_timestamp(1_000 + VOTING_PERIOD_SECS + 1);
    vault.settle_buyout(&vid);

    let state = vault.get_vault(&vid).unwrap();
    assert_eq!(state.status, VaultStatus::Completed);

    // NFT is now owned by buyer.
    assert_eq!(nft.owner_of(&1u32), buyer);

    // Holders claim their proportional proceeds from the 12_000 offer.
    // owner: 600/1000 * 12_000 = 7_200
    // holder: 400/1000 * 12_000 = 4_800
    vault.claim_proceeds(&owner, &vid);
    vault.claim_proceeds(&holder, &vid);

    assert_eq!(payment_token.balance(&owner), 7_200);
    assert_eq!(payment_token.balance(&holder), 4_800);

    // Vault contract holds nothing (fully distributed).
    assert_eq!(payment_token.balance(&vault_addr), 0);

    // Fractions burned after claim.
    assert_eq!(vault.balance_of(&vid, &owner), 0);
    assert_eq!(vault.balance_of(&vid, &holder), 0);
}

/// Buyout fails when approval is below 50 % of total fractions; offer refunded.
#[test]
fn test_buyout_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let holder = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (payment_token_id, payment_token, payment_admin) = setup_token(&env, &admin);
    payment_admin.mint(&buyer, &15_000i128);

    let (nft_addr, nft) = setup_nft(&env);
    nft.set_owner(&1u32, &owner);

    let (_, vault) = setup_vault(&env);

    // 1_000 fractions; owner keeps 600, holder gets 400.
    let vid = vault.fractionalize(
        &owner,
        &nft_addr,
        &1u32,
        &1_000i128,
        &10_000i128,
        &payment_token_id,
    );
    vault.transfer_fraction(&owner, &vid, &holder, &400i128);

    env.ledger().set_timestamp(1_000);
    vault.initiate_buyout(&buyer, &vid, &12_000i128);

    // Only holder (400 fractions) votes against → 0 for, 400 against.
    // 0 * 2 <= 1_000 → rejected.
    vault.vote_buyout(&holder, &vid, &false);

    env.ledger().set_timestamp(1_000 + VOTING_PERIOD_SECS + 1);
    vault.settle_buyout(&vid);

    let state = vault.get_vault(&vid).unwrap();
    // Vault reverts to Active after rejection.
    assert_eq!(state.status, VaultStatus::Active);

    // NFT still locked in vault.
    let (vault_addr, _) = setup_vault(&env); // re-derive for address
    // (We compare against the vault client's address)
    // Instead, directly check buyer does NOT own NFT.
    assert_ne!(nft.owner_of(&1u32), buyer);

    // Buyer was refunded the full 12_000.
    assert_eq!(payment_token.balance(&buyer), 15_000);
}

/// A fraction holder cannot vote twice on the same buyout.
#[test]
#[should_panic(expected = "already_voted")]
fn test_double_vote_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (payment_token_id, _, payment_admin) = setup_token(&env, &admin);
    payment_admin.mint(&buyer, &20_000i128);

    let (nft_addr, nft) = setup_nft(&env);
    nft.set_owner(&1u32, &owner);

    let (_, vault) = setup_vault(&env);
    let vid = vault.fractionalize(
        &owner,
        &nft_addr,
        &1u32,
        &1_000i128,
        &10_000i128,
        &payment_token_id,
    );

    env.ledger().set_timestamp(1_000);
    vault.initiate_buyout(&buyer, &vid, &12_000i128);

    vault.vote_buyout(&owner, &vid, &true);
    // Second vote from the same address → panic.
    vault.vote_buyout(&owner, &vid, &false);
}

/// Addresses without fractions cannot vote.
#[test]
#[should_panic(expected = "no_fractions")]
fn test_non_holder_cannot_vote() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let stranger = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (payment_token_id, _, payment_admin) = setup_token(&env, &admin);
    payment_admin.mint(&buyer, &20_000i128);

    let (nft_addr, nft) = setup_nft(&env);
    nft.set_owner(&1u32, &owner);

    let (_, vault) = setup_vault(&env);
    let vid = vault.fractionalize(
        &owner,
        &nft_addr,
        &1u32,
        &1_000i128,
        &10_000i128,
        &payment_token_id,
    );

    env.ledger().set_timestamp(1_000);
    vault.initiate_buyout(&buyer, &vid, &12_000i128);

    // Stranger holds 0 fractions → should panic.
    vault.vote_buyout(&stranger, &vid, &true);
}

/// get_vault returns the NFT lock status, fraction supply, and buyout state.
#[test]
fn test_get_vault_state() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (payment_token_id, _, payment_admin) = setup_token(&env, &admin);
    payment_admin.mint(&buyer, &20_000i128);

    let (nft_addr, nft) = setup_nft(&env);
    nft.set_owner(&1u32, &owner);

    let (_, vault) = setup_vault(&env);
    let vid = vault.fractionalize(
        &owner,
        &nft_addr,
        &1u32,
        &500i128,
        &5_000i128,
        &payment_token_id,
    );

    let state = vault.get_vault(&vid).unwrap();
    assert_eq!(state.nft_id, 1);
    assert_eq!(state.total_fractions, 500);
    assert_eq!(state.buyout_price, 5_000);
    assert_eq!(state.status, VaultStatus::Active);
    assert!(state.buyout_buyer.is_none());

    env.ledger().set_timestamp(100);
    vault.initiate_buyout(&buyer, &vid, &6_000i128);

    let state = vault.get_vault(&vid).unwrap();
    assert_eq!(state.status, VaultStatus::BuyoutPending);
    assert_eq!(state.buyout_offer_price, 6_000);
    assert_eq!(state.buyout_buyer, Some(buyer.clone()));
    assert_eq!(state.buyout_deadline, 100 + VOTING_PERIOD_SECS);
}
