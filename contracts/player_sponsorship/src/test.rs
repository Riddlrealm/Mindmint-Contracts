#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl, symbol_short, token,
    testutils::Address as _,
    Address, Env,
};

#[contract]
struct MockProof;

#[contractimpl]
impl MockProof {
    pub fn set_count(env: Env, player: Address, activity_type: u32, count: u32) {
        env.storage()
            .persistent()
            .set(&(symbol_short!("CNT"), player, activity_type), &count);
    }

    pub fn get_activity_count(env: Env, player: Address, activity_type: u32) -> u32 {
        env.storage()
            .persistent()
            .get(&(symbol_short!("CNT"), player, activity_type))
            .unwrap_or(0)
    }
}

#[contract]
struct MockLeaderboard;

#[contractimpl]
impl MockLeaderboard {
    pub fn set_rank(env: Env, player: Address, period: leaderboard_types::TimePeriod, rank: u32) {
        env.storage()
            .persistent()
            .set(&(symbol_short!("RANK"), player, period as u32), &rank);
    }

    pub fn get_player_rank(env: Env, player: Address, period: leaderboard_types::TimePeriod) -> u32 {
        env.storage()
            .persistent()
            .get(&(symbol_short!("RANK"), player, period as u32))
            .unwrap_or(0)
    }
}

fn setup(env: &Env) -> (
    PlayerSponsorshipContractClient<'_>,
    token::Client<'_>,
    Address,
    Address,
) {
    let admin = Address::generate(env);

    let proof_id = env.register_contract(None, MockProof);
    let leaderboard_id = env.register_contract(None, MockLeaderboard);

    let sponsorship_id = env.register_contract(None, PlayerSponsorshipContract);
    let client = PlayerSponsorshipContractClient::new(env, &sponsorship_id);

    client.initialize(&admin, &proof_id, &leaderboard_id);

    let token_admin = Address::generate(env);
    let token_addr = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_client = token::Client::new(env, &token_addr);

    (client, token_client, proof_id, leaderboard_id)
}

fn mint(env: &Env, token: &token::Client<'_>, to: &Address, amount: i128) {
    let asset_client = token::StellarAssetClient::new(env, &token.address);
    asset_client.mint(to, &amount);
}

#[test]
fn test_create_deal_escrows_full_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, token, _proof_id, _lb_id) = setup(&env);

    let sponsor = Address::generate(&env);
    let player = Address::generate(&env);
    mint(&env, &token, &sponsor, 1_000);

    let mut milestones: Vec<Milestone> = Vec::new(&env);
    milestones.push_back(Milestone {
        condition: MilestoneCondition::PuzzlesSolved(1),
        reward_amount: 400,
        claimed: false,
    });
    milestones.push_back(Milestone {
        condition: MilestoneCondition::PuzzlesSolved(2),
        reward_amount: 600,
        claimed: false,
    });

    let deal_id = client.create_deal(&sponsor, &player, &token.address, &milestones, &1_000);

    assert_eq!(deal_id, 1);
    assert_eq!(token.balance(&sponsor), 0);
    assert_eq!(token.balance(&client.address), 1_000);

    let deal = client.get_deal(&deal_id).unwrap();
    assert_eq!(deal.sponsor, sponsor);
    assert_eq!(deal.player, player);
    assert_eq!(deal.total_amount, 1_000);
    assert_eq!(deal.released, 0);
    assert_eq!(deal.milestones.len(), 2);
}

#[test]
fn test_valid_milestone_claim_releases_payment() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, token, proof_id, _lb_id) = setup(&env);

    let sponsor = Address::generate(&env);
    let player = Address::generate(&env);
    mint(&env, &token, &sponsor, 1_000);

    let mut milestones: Vec<Milestone> = Vec::new(&env);
    milestones.push_back(Milestone {
        condition: MilestoneCondition::PuzzlesSolved(3),
        reward_amount: 1_000,
        claimed: false,
    });

    let deal_id = client.create_deal(&sponsor, &player, &token.address, &milestones, &1_000);

    let proof_client = MockProofClient::new(&env, &proof_id);
    proof_client.set_count(&player, &0u32, &3u32);

    client.claim_milestone(&player, &deal_id, &0u32);

    assert_eq!(token.balance(&player), 1_000);
    assert_eq!(token.balance(&client.address), 0);

    let deal = client.get_deal(&deal_id).unwrap();
    assert_eq!(deal.released, 1_000);
    assert!(deal.milestones.get(0).unwrap().claimed);
}

#[test]
#[should_panic(expected = "Milestone condition not met")]
fn test_invalid_claim_condition_unmet() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, token, proof_id, _lb_id) = setup(&env);

    let sponsor = Address::generate(&env);
    let player = Address::generate(&env);
    mint(&env, &token, &sponsor, 500);

    let mut milestones: Vec<Milestone> = Vec::new(&env);
    milestones.push_back(Milestone {
        condition: MilestoneCondition::PuzzlesSolved(10),
        reward_amount: 500,
        claimed: false,
    });

    let deal_id = client.create_deal(&sponsor, &player, &token.address, &milestones, &500);

    let proof_client = MockProofClient::new(&env, &proof_id);
    proof_client.set_count(&player, &0u32, &2u32);

    client.claim_milestone(&player, &deal_id, &0u32);
}

#[test]
fn test_sponsor_cancel_reclaims_unclaimed() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, token, proof_id, _lb_id) = setup(&env);

    let sponsor = Address::generate(&env);
    let player = Address::generate(&env);
    mint(&env, &token, &sponsor, 1_000);

    let mut milestones: Vec<Milestone> = Vec::new(&env);
    milestones.push_back(Milestone {
        condition: MilestoneCondition::PuzzlesSolved(1),
        reward_amount: 400,
        claimed: false,
    });
    milestones.push_back(Milestone {
        condition: MilestoneCondition::PuzzlesSolved(2),
        reward_amount: 600,
        claimed: false,
    });

    let deal_id = client.create_deal(&sponsor, &player, &token.address, &milestones, &1_000);

    let proof_client = MockProofClient::new(&env, &proof_id);
    proof_client.set_count(&player, &0u32, &1u32);

    client.claim_milestone(&player, &deal_id, &0u32);

    assert_eq!(token.balance(&player), 400);
    assert_eq!(token.balance(&client.address), 600);

    client.cancel_deal(&sponsor, &deal_id);

    assert_eq!(token.balance(&sponsor), 600);
    assert_eq!(token.balance(&client.address), 0);

    let deal = client.get_deal(&deal_id).unwrap();
    assert!(deal.cancelled);
}
