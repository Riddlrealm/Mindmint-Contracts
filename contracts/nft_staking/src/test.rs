#![cfg(test)]

use super::*;
use reward_token::{RewardToken as RewardTokenContract, RewardTokenClient as RewardTokenContractClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String,
};

// ─────────────────────────────────────────────────────────────
// Mock NFT contract with rarity + transfer + burn restrictions
// ─────────────────────────────────────────────────────────────

#[contract]
struct MockAchievementNft;

#[contracttype]
enum MockKey {
    Admin,
    Owner(u32),
    Rarity(u32),
}

#[contractimpl]
impl MockAchievementNft {
    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().instance().set(&MockKey::Admin, &admin);
    }

    pub fn mint(env: Env, admin: Address, to: Address, token_id: u32, rarity: u32) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&MockKey::Admin).unwrap();
        if admin != stored_admin {
            panic!("not admin");
        }

        env.storage().persistent().set(&MockKey::Owner(token_id), &to);
        env.storage().persistent().set(&MockKey::Rarity(token_id), &rarity);
    }

    pub fn owner_of(env: Env, token_id: u32) -> Address {
        env.storage()
            .persistent()
            .get(&MockKey::Owner(token_id))
            .unwrap()
    }

    pub fn transfer(env: Env, from: Address, to: Address, token_id: u32) {
        from.require_auth();
        let owner = Self::owner_of(env.clone(), token_id);
        if owner != from {
            panic!("not owner");
        }
        env.storage().persistent().set(&MockKey::Owner(token_id), &to);
    }

    pub fn burn(env: Env, owner: Address, token_id: u32) {
        owner.require_auth();
        let current = Self::owner_of(env.clone(), token_id);
        if current != owner {
            panic!("not owner");
        }
        env.storage().persistent().remove(&MockKey::Owner(token_id));
        env.storage().persistent().remove(&MockKey::Rarity(token_id));
    }
}

fn set_ledger(env: &Env, sequence: u32, timestamp: u64) {
    env.ledger().with_mut(|li| {
        li.sequence_number = sequence;
        li.timestamp = timestamp;
    });
}

struct Setup {
    env: Env,
    admin: Address,
    staker: Address,
    other: Address,
    staking_id: Address,
    staking: NftStakingContractClient<'static>,
    nft: MockAchievementNftClient<'static>,
    reward_token: RewardTokenContractClient<'static>,
}

fn setup() -> Setup {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let staker = Address::generate(&env);
    let other = Address::generate(&env);

    // Reward token
    let reward_token_id = env.register_contract(None, RewardTokenContract);
    let reward_token = RewardTokenContractClient::new(&env, &reward_token_id);
    reward_token.initialize(
        &admin,
        &String::from_str(&env, "Reward"),
        &String::from_str(&env, "RWD"),
        &6u32,
    );

    // Mock NFT
    let nft_id = env.register_contract(None, MockAchievementNft);
    let nft = MockAchievementNftClient::new(&env, &nft_id);
    nft.initialize(&admin);

    // Staking contract
    let staking_id = env.register_contract(None, NftStakingContract);
    let staking = NftStakingContractClient::new(&env, &staking_id);
    staking.initialize(&admin, &nft_id, &reward_token_id);

    // Allow staking contract to mint rewards
    reward_token.authorize_minter(&staking_id);

    Setup {
        env,
        admin,
        staker,
        other,
        staking_id,
        staking,
        nft,
        reward_token,
    }
}

#[test]
fn stake_locks_nft_and_records_position() {
    let s = setup();
    set_ledger(&s.env, 10, 1_000);

    s.staking.set_rarity_config(&s.admin, &1u32, &5i128);
    s.staking.set_token_rarity(&s.admin, &1u32, &1u32);

    s.nft.mint(&s.admin, &s.staker, &1u32, &1u32);
    assert_eq!(s.nft.owner_of(&1u32), s.staker);

    s.staking.stake(&s.staker, &1u32);

    // NFT is locked in contract while staked.
    assert_eq!(s.nft.owner_of(&1u32), s.staking_id);

    let view = s.staking.get_position(&1u32);
    assert_eq!(view.position.token_id, 1);
    assert_eq!(view.position.staker, s.staker);
    assert_eq!(view.position.rarity, 1);
    assert_eq!(view.position.last_claim_ledger, 10);
    assert_eq!(view.pending_rewards, 0);
    assert_eq!(view.days_staked, 0);
}

#[test]
#[should_panic(expected = "not owner")]
fn transfer_blocked_while_staked() {
    let s = setup();
    set_ledger(&s.env, 10, 1_000);

    s.staking.set_rarity_config(&s.admin, &1u32, &5i128);
    s.staking.set_token_rarity(&s.admin, &1u32, &1u32);

    s.nft.mint(&s.admin, &s.staker, &1u32, &1u32);
    s.staking.stake(&s.staker, &1u32);

    // Staker is no longer the owner, so transfer must fail.
    s.nft.transfer(&s.staker, &s.other, &1u32);
}

#[test]
#[should_panic(expected = "not owner")]
fn burn_blocked_while_staked() {
    let s = setup();
    set_ledger(&s.env, 10, 1_000);

    s.staking.set_rarity_config(&s.admin, &1u32, &5i128);
    s.staking.set_token_rarity(&s.admin, &1u32, &1u32);

    s.nft.mint(&s.admin, &s.staker, &1u32, &1u32);
    s.staking.stake(&s.staker, &1u32);

    // Staker is no longer the owner, so burn must fail.
    s.nft.burn(&s.staker, &1u32);
}

#[test]
fn rewards_accrue_per_ledger_and_claim_without_unstaking() {
    let s = setup();
    set_ledger(&s.env, 10, 1_000);

    s.staking.set_rarity_config(&s.admin, &1u32, &10i128);
    s.staking.set_token_rarity(&s.admin, &1u32, &1u32);
    s.nft.mint(&s.admin, &s.staker, &1u32, &1u32);
    s.staking.stake(&s.staker, &1u32);

    set_ledger(&s.env, 15, 1_050);
    assert_eq!(s.staking.pending_rewards(&1u32), 50);

    let claimed1 = s.staking.claim_rewards(&s.staker, &1u32);
    assert_eq!(claimed1, 50);
    assert_eq!(s.reward_token.balance(&s.staker), 50);

    // Claim again later without unstaking.
    set_ledger(&s.env, 18, 1_100);
    assert_eq!(s.staking.pending_rewards(&1u32), 30);

    let claimed2 = s.staking.claim_rewards(&s.staker, &1u32);
    assert_eq!(claimed2, 30);
    assert_eq!(s.reward_token.balance(&s.staker), 80);

    let view = s.staking.get_position(&1u32);
    assert_eq!(view.pending_rewards, 0);
    assert_eq!(view.position.total_claimed, 80);
    assert_eq!(view.position.last_claim_ledger, 18);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn unstake_before_48h_rejected() {
    let s = setup();
    set_ledger(&s.env, 10, 1_000);

    s.staking.set_rarity_config(&s.admin, &1u32, &2i128);
    s.staking.set_token_rarity(&s.admin, &1u32, &1u32);
    s.nft.mint(&s.admin, &s.staker, &1u32, &1u32);
    s.staking.stake(&s.staker, &1u32);

    set_ledger(&s.env, 11, 1_000 + MIN_STAKE_SECONDS - 1);
    s.staking.unstake(&s.staker, &1u32);
}

#[test]
fn unstake_after_48h_returns_nft_and_claims_rewards() {
    let s = setup();
    set_ledger(&s.env, 10, 1_000);

    s.staking.set_rarity_config(&s.admin, &1u32, &2i128);
    s.staking.set_token_rarity(&s.admin, &1u32, &1u32);
    s.nft.mint(&s.admin, &s.staker, &1u32, &1u32);
    s.staking.stake(&s.staker, &1u32);

    set_ledger(&s.env, 20, 1_000 + MIN_STAKE_SECONDS + 1);

    let pending = s.staking.unstake(&s.staker, &1u32);
    assert_eq!(pending, 20); // (20 - 10) * 2
    assert_eq!(s.reward_token.balance(&s.staker), 20);
    assert_eq!(s.nft.owner_of(&1u32), s.staker);
}

#[test]
fn rarity_rate_differences_affect_rewards() {
    let s = setup();
    set_ledger(&s.env, 100, 1_000);

    s.staking.set_rarity_config(&s.admin, &1u32, &5i128);
    s.staking.set_rarity_config(&s.admin, &2u32, &20i128);
    s.staking.set_token_rarity(&s.admin, &1u32, &1u32);
    s.staking.set_token_rarity(&s.admin, &2u32, &2u32);

    s.nft.mint(&s.admin, &s.staker, &1u32, &1u32);
    s.nft.mint(&s.admin, &s.staker, &2u32, &2u32);
    s.staking.stake(&s.staker, &1u32);
    s.staking.stake(&s.staker, &2u32);

    set_ledger(&s.env, 110, 1_050);

    let p1 = s.staking.pending_rewards(&1u32);
    let p2 = s.staking.pending_rewards(&2u32);
    assert_eq!(p1, 50);  // (110 - 100) * 5
    assert_eq!(p2, 200); // (110 - 100) * 20
}
