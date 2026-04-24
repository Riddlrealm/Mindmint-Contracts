#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String, Vec,
};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let token_addr = env.register_stellar_asset_contract(admin.clone());
    (
        TokenClient::new(env, &token_addr),
        StellarAssetClient::new(env, &token_addr),
    )
}

fn milestone(env: &Env, description: &str, amount: i128) -> GrantMilestone {
    GrantMilestone {
        description: String::from_str(env, description),
        amount,
        claimed: false,
        verified: false,
    }
}

fn setup() -> (Env, CommunityGrantContractClient<'static>, TokenClient<'static>, StellarAssetClient<'static>, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let applicant = Address::generate(&env);
    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);

    let (token_client, token_admin) = create_token(&env, &admin);

    token_admin.mint(&voter1, &700);
    token_admin.mint(&voter2, &500);

    let contract_id = env.register_contract(None, CommunityGrantContract);
    let client = CommunityGrantContractClient::new(&env, &contract_id);

    client.initialize(&admin, &token_client.address, &1000);

    (
        env,
        client,
        token_client,
        token_admin,
        admin,
        applicant,
        voter1,
        voter2,
    )
}

#[test]
fn test_submit_vote_approve_verify_claim_flow() {
    let (env, client, token_client, token_admin, admin, applicant, voter1, voter2) = setup();

    let mut milestones = Vec::new(&env);
    milestones.push_back(milestone(&env, "m1", 400));
    milestones.push_back(milestone(&env, "m2", 600));

    let proposal_id = client.submit_proposal(&applicant, &1000, &milestones);

    client.vote(&voter1, &proposal_id, &true);
    client.vote(&voter2, &proposal_id, &false);

    env.ledger().with_mut(|li| {
        li.timestamp += VOTING_WINDOW_SECONDS + 1;
    });

    let proposal = client.get_proposal(&proposal_id);
    assert_eq!(proposal.status, ProposalStatus::Approved);
    assert_eq!(proposal.votes_for, 700);
    assert_eq!(proposal.votes_against, 500);
    assert_eq!(proposal.approved_at, Some(proposal.voting_ends_at));

    token_admin.mint(&client.address, &1000);

    client.verify_milestone(&admin, &proposal_id, &0);
    let proposal_after_verify = client.get_proposal(&proposal_id);
    assert_eq!(proposal_after_verify.milestones.get(0).unwrap().verified, true);

    let before = token_client.balance(&applicant);
    client.claim_milestone(&applicant, &proposal_id, &0);
    let after = token_client.balance(&applicant);
    assert_eq!(after - before, 400);

    let proposal_after_claim = client.get_proposal(&proposal_id);
    assert_eq!(proposal_after_claim.milestones.get(0).unwrap().claimed, true);
}

#[test]
fn test_quorum_required_for_approval() {
    let (env, client, _token_client, _token_admin, _admin, applicant, voter1, _voter2) = setup();

    let mut milestones = Vec::new(&env);
    milestones.push_back(milestone(&env, "m1", 1000));

    let proposal_id = client.submit_proposal(&applicant, &1000, &milestones);

    // Only 700 votes cast, quorum is 1000.
    client.vote(&voter1, &proposal_id, &true);

    env.ledger().with_mut(|li| {
        li.timestamp += VOTING_WINDOW_SECONDS + 1;
    });

    let proposal = client.get_proposal(&proposal_id);
    assert_eq!(proposal.status, ProposalStatus::Rejected);
    assert_eq!(proposal.votes_for, 700);
    assert_eq!(proposal.votes_against, 0);
}

#[test]
#[should_panic]
fn test_voting_window_enforced_at_7_days() {
    let (env, client, _token_client, _token_admin, _admin, applicant, voter1, _voter2) = setup();

    let mut milestones = Vec::new(&env);
    milestones.push_back(milestone(&env, "m1", 1000));

    let proposal_id = client.submit_proposal(&applicant, &1000, &milestones);

    env.ledger().with_mut(|li| {
        li.timestamp += VOTING_WINDOW_SECONDS + 1;
    });

    client.vote(&voter1, &proposal_id, &true);
}

#[test]
#[should_panic(expected = "Milestone already claimed")]
fn test_double_claim_rejected() {
    let (env, client, _token_client, token_admin, admin, applicant, voter1, voter2) = setup();

    let mut milestones = Vec::new(&env);
    milestones.push_back(milestone(&env, "m1", 1000));

    let proposal_id = client.submit_proposal(&applicant, &1000, &milestones);

    client.vote(&voter1, &proposal_id, &true);
    client.vote(&voter2, &proposal_id, &false);

    env.ledger().with_mut(|li| {
        li.timestamp += VOTING_WINDOW_SECONDS + 1;
    });

    token_admin.mint(&client.address, &1000);
    client.verify_milestone(&admin, &proposal_id, &0);

    client.claim_milestone(&applicant, &proposal_id, &0);
    client.claim_milestone(&applicant, &proposal_id, &0);
}

#[test]
#[should_panic(expected = "Milestone not verified")]
fn test_claim_requires_admin_verification() {
    let (env, client, _token_client, token_admin, _admin, applicant, voter1, voter2) = setup();

    let mut milestones = Vec::new(&env);
    milestones.push_back(milestone(&env, "m1", 1000));

    let proposal_id = client.submit_proposal(&applicant, &1000, &milestones);

    client.vote(&voter1, &proposal_id, &true);
    client.vote(&voter2, &proposal_id, &false);

    env.ledger().with_mut(|li| {
        li.timestamp += VOTING_WINDOW_SECONDS + 1;
    });

    token_admin.mint(&client.address, &1000);
    client.claim_milestone(&applicant, &proposal_id, &0);
}
