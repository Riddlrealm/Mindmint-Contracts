#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl,
    testutils::Address as _,
    token, Address, Env,
};

// ─────────────────────────────────────────────
// Mock NFT contract
// ─────────────────────────────────────────────

#[contract]
pub struct MockNft;

#[contractimpl]
impl MockNft {
    pub fn owner_of(env: Env, token_id: u32) -> Address {
        env.storage()
            .persistent()
            .get(&token_id)
            .unwrap_or_else(|| panic!("token_not_found"))
    }

    pub fn transfer(env: Env, from: Address, to: Address, token_id: u32) {
        from.require_auth();
        let owner: Address = env
            .storage()
            .persistent()
            .get(&token_id)
            .unwrap_or_else(|| panic!("token_not_found"));
        if owner != from {
            panic!("not_owner");
        }
        env.storage().persistent().set(&token_id, &to);
    }

    pub fn set_owner(env: Env, token_id: u32, owner: Address) {
        env.storage().persistent().set(&token_id, &owner);
    }
}

// ─────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────

fn setup_payment_token<'a>(
    env: &'a Env,
    admin: &'a Address,
) -> (Address, token::Client<'a>, token::StellarAssetClient<'a>) {
    let addr = env.register_stellar_asset_contract_v2(admin.clone()).address();
    let client = token::Client::new(env, &addr);
    let sac = token::StellarAssetClient::new(env, &addr);
    (addr, client, sac)
}

/// Register a MockNft contract and mint `count` tokens to `owner`.
/// Returns (nft_contract_address, client, Vec<TokenRef>).
fn setup_nft_pool<'a>(
    env: &'a Env,
    owner: &Address,
    count: u32,
) -> (Address, MockNftClient<'a>, Vec<TokenRef>) {
    let nft_addr = env.register_contract(None, MockNft);
    let nft = MockNftClient::new(env, &nft_addr);
    let mut pool: Vec<TokenRef> = Vec::new(env);
    for i in 1..=count {
        nft.set_owner(&i, owner);
        pool.push_back(TokenRef {
            nft_contract: nft_addr.clone(),
            token_id: i,
        });
    }
    (nft_addr, nft, pool)
}

fn setup_bundle_contract(env: &Env) -> (Address, NftBundleContractClient) {
    let addr = env.register_contract(None, NftBundleContract);
    let client = NftBundleContractClient::new(env, &addr);
    (addr, client)
}

// ─────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────

/// Creator can create a fixed bundle; NFTs are locked in the contract.
#[test]
fn test_create_fixed_bundle() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let (payment_addr, _, _) = setup_payment_token(&env, &admin);
    let (nft_addr, nft, pool) = setup_nft_pool(&env, &creator, 4);
    let (bundle_addr, bundle) = setup_bundle_contract(&env);

    let id = bundle.create_bundle(
        &creator,
        &PackType::Fixed,
        &pool,
        &2u32,       // 2 NFTs per pack → 2 packs
        &1000i128,
        &payment_addr,
    );

    let b = bundle.get_bundle(&id).unwrap();
    assert_eq!(b.packs_remaining, 2);
    assert_eq!(b.pack_type, PackType::Fixed);
    assert_eq!(b.price, 1000);
    assert_eq!(b.nft_pool.len(), 4);
    assert_eq!(b.status, BundleStatus::Active);

    // NFTs should now be owned by the bundle contract.
    assert_eq!(nft.owner_of(&1u32), bundle_addr);
    assert_eq!(nft.owner_of(&2u32), bundle_addr);
}

/// Creator can create a blind bundle.
#[test]
fn test_create_blind_bundle() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let (payment_addr, _, _) = setup_payment_token(&env, &admin);
    let (_, _, pool) = setup_nft_pool(&env, &creator, 6);
    let (_, bundle) = setup_bundle_contract(&env);

    let id = bundle.create_bundle(
        &creator,
        &PackType::Blind,
        &pool,
        &3u32,       // 3 NFTs per pack → 2 packs
        &500i128,
        &payment_addr,
    );

    let b = bundle.get_bundle(&id).unwrap();
    assert_eq!(b.packs_remaining, 2);
    assert_eq!(b.pack_type, PackType::Blind);
}

/// Fixed pack delivers the first N NFTs in pool order.
#[test]
fn test_purchase_fixed_pack() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    let (payment_addr, _, sac) = setup_payment_token(&env, &admin);
    sac.mint(&buyer, &5000i128);

    let (nft_addr, nft, pool) = setup_nft_pool(&env, &creator, 4);
    let (_, bundle) = setup_bundle_contract(&env);

    let id = bundle.create_bundle(
        &creator,
        &PackType::Fixed,
        &pool,
        &2u32,
        &1000i128,
        &payment_addr,
    );

    let received = bundle.purchase_pack(&buyer, &id);

    // Fixed: first 2 tokens (id 1 and 2) delivered.
    assert_eq!(received.len(), 2);
    assert_eq!(received.get(0).unwrap().token_id, 1);
    assert_eq!(received.get(1).unwrap().token_id, 2);

    // Buyer now owns those NFTs.
    assert_eq!(nft.owner_of(&1u32), buyer);
    assert_eq!(nft.owner_of(&2u32), buyer);

    // packs_remaining decremented.
    let b = bundle.get_bundle(&id).unwrap();
    assert_eq!(b.packs_remaining, 1);
    assert_eq!(b.nft_pool.len(), 2);
}

/// Blind pack draws without repeats; pool shrinks correctly.
#[test]
fn test_purchase_blind_pack_no_repeats() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    let (payment_addr, _, sac) = setup_payment_token(&env, &admin);
    sac.mint(&buyer, &5000i128);

    let (_, nft, pool) = setup_nft_pool(&env, &creator, 6);
    let (_, bundle) = setup_bundle_contract(&env);

    let id = bundle.create_bundle(
        &creator,
        &PackType::Blind,
        &pool,
        &3u32,
        &500i128,
        &payment_addr,
    );

    let received = bundle.purchase_pack(&buyer, &id);
    assert_eq!(received.len(), 3);

    // All token ids must be distinct.
    let t0 = received.get(0).unwrap().token_id;
    let t1 = received.get(1).unwrap().token_id;
    let t2 = received.get(2).unwrap().token_id;
    assert_ne!(t0, t1);
    assert_ne!(t1, t2);
    assert_ne!(t0, t2);

    // Pool shrinks by 3.
    let b = bundle.get_bundle(&id).unwrap();
    assert_eq!(b.nft_pool.len(), 3);
    assert_eq!(b.packs_remaining, 1);
}

/// Bundle auto-closes when the last pack is purchased.
#[test]
fn test_bundle_closes_when_empty() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    let (payment_addr, _, sac) = setup_payment_token(&env, &admin);
    sac.mint(&buyer, &5000i128);

    let (_, _, pool) = setup_nft_pool(&env, &creator, 2);
    let (_, bundle) = setup_bundle_contract(&env);

    let id = bundle.create_bundle(
        &creator,
        &PackType::Fixed,
        &pool,
        &2u32,   // 1 pack total
        &1000i128,
        &payment_addr,
    );

    bundle.purchase_pack(&buyer, &id);

    let b = bundle.get_bundle(&id).unwrap();
    assert_eq!(b.packs_remaining, 0);
    assert_eq!(b.status, BundleStatus::Closed);
}

/// Purchasing from an empty / closed bundle panics.
#[test]
#[should_panic(expected = "bundle_not_active")]
fn test_purchase_from_closed_bundle_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    let (payment_addr, _, sac) = setup_payment_token(&env, &admin);
    sac.mint(&buyer, &5000i128);

    let (_, _, pool) = setup_nft_pool(&env, &creator, 2);
    let (_, bundle) = setup_bundle_contract(&env);

    let id = bundle.create_bundle(
        &creator,
        &PackType::Fixed,
        &pool,
        &2u32,
        &1000i128,
        &payment_addr,
    );

    // Buy the only pack → bundle closes.
    bundle.purchase_pack(&buyer, &id);
    // Second attempt must panic.
    bundle.purchase_pack(&buyer, &id);
}

/// Creator can cancel an active bundle and withdraw unsold NFTs.
#[test]
fn test_creator_cancel_and_withdraw() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let (payment_addr, _, _) = setup_payment_token(&env, &admin);
    let (_, nft, pool) = setup_nft_pool(&env, &creator, 4);
    let (bundle_addr, bundle) = setup_bundle_contract(&env);

    let id = bundle.create_bundle(
        &creator,
        &PackType::Fixed,
        &pool,
        &2u32,
        &1000i128,
        &payment_addr,
    );

    // NFTs are in the contract.
    assert_eq!(nft.owner_of(&1u32), bundle_addr);

    bundle.cancel_bundle(&creator, &id);

    let b = bundle.get_bundle(&id).unwrap();
    assert_eq!(b.status, BundleStatus::Cancelled);

    bundle.withdraw_unsold(&creator, &id);

    // All NFTs returned to creator.
    assert_eq!(nft.owner_of(&1u32), creator);
    assert_eq!(nft.owner_of(&2u32), creator);
    assert_eq!(nft.owner_of(&3u32), creator);
    assert_eq!(nft.owner_of(&4u32), creator);

    // Pool is empty after withdrawal.
    let b = bundle.get_bundle(&id).unwrap();
    assert_eq!(b.nft_pool.len(), 0);
}

/// Creator can withdraw proceeds after packs are sold.
#[test]
fn test_creator_withdraw_proceeds() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let buyer = Address::generate(&env);
    let (payment_addr, payment, sac) = setup_payment_token(&env, &admin);
    sac.mint(&buyer, &5000i128);

    let (_, _, pool) = setup_nft_pool(&env, &creator, 2);
    let (_, bundle) = setup_bundle_contract(&env);

    let id = bundle.create_bundle(
        &creator,
        &PackType::Fixed,
        &pool,
        &2u32,
        &1000i128,
        &payment_addr,
    );

    bundle.purchase_pack(&buyer, &id);

    let creator_balance_before = payment.balance(&creator);
    bundle.withdraw_proceeds(&creator, &id);
    let creator_balance_after = payment.balance(&creator);

    assert_eq!(creator_balance_after - creator_balance_before, 1000);
}

/// Non-creator cannot cancel a bundle.
#[test]
#[should_panic(expected = "not_creator")]
fn test_non_creator_cannot_cancel() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let attacker = Address::generate(&env);
    let (payment_addr, _, _) = setup_payment_token(&env, &admin);
    let (_, _, pool) = setup_nft_pool(&env, &creator, 2);
    let (_, bundle) = setup_bundle_contract(&env);

    let id = bundle.create_bundle(
        &creator,
        &PackType::Fixed,
        &pool,
        &2u32,
        &1000i128,
        &payment_addr,
    );

    bundle.cancel_bundle(&attacker, &id);
}

/// get_bundle returns correct metadata.
#[test]
fn test_get_bundle_metadata() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let (payment_addr, _, _) = setup_payment_token(&env, &admin);
    let (_, _, pool) = setup_nft_pool(&env, &creator, 6);
    let (_, bundle) = setup_bundle_contract(&env);

    let id = bundle.create_bundle(
        &creator,
        &PackType::Blind,
        &pool,
        &3u32,
        &750i128,
        &payment_addr,
    );

    let b = bundle.get_bundle(&id).unwrap();
    assert_eq!(b.pack_type, PackType::Blind);
    assert_eq!(b.price, 750);
    assert_eq!(b.packs_remaining, 2);
    assert_eq!(b.nft_pool.len(), 6);
}
