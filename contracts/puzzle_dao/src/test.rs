#![cfg(test)]

use super::*;
use crate::types::{MembershipTier, ProposalCategory, VoteType};
use reward_token::{RewardToken, RewardTokenClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String, Symbol, Vec,
};

#[test]
fn test_dao_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &100,    // voting_delay
        &604800, // voting_period (7 days)
        &1000,   // proposal_threshold
        &10,     // quorum_percentage
        &86400,  // execution_delay
        &50,     // emergency_quorum_percentage
    );

    // Test that initialization worked
    let treasury_info = client.get_treasury_balance();
    assert_eq!(treasury_info.total_balance, 0);
    assert_eq!(treasury_info.allocated_funds, 0);
}

#[test]
fn test_membership_joining() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);
    let member = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &100,
        &604800,
        &1000,
        &10,
        &86400,
        &50,
    );

    // Setup token
    let token_client = RewardTokenClient::new(&env, &token_id);
    let admin = Address::generate(&env);
    token_client.initialize(
        &admin,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
        &6,
    );
    token_client.mint(&admin, &member, &10000);

    // Member joins DAO
    client.join_dao(&member, &5000);

    // Check membership info
    let member_info = client.get_member_info(&member);
    assert_eq!(member_info.address, member);
    assert_eq!(member_info.tier, MembershipTier::Active);
    assert!(member_info.is_active);

    // Check voting power
    let voting_power = client.get_user_voting_power(&member);
    assert_eq!(voting_power, 5000);
}

#[test]
fn test_membership_upgrade() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);
    let member = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &100,
        &604800,
        &1000,
        &10,
        &86400,
        &50,
    );

    let token_client = RewardTokenClient::new(&env, &token_id);
    let admin = Address::generate(&env);
    token_client.initialize(
        &admin,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
        &6,
    );
    token_client.mint(&admin, &member, &30000);

    // Join as Basic member
    client.join_dao(&member, &3000);
    let member_info = client.get_member_info(&member);
    assert_eq!(member_info.tier, MembershipTier::Basic);

    // Upgrade to Premium
    client.upgrade_membership(&member, &17000);
    let member_info = client.get_member_info(&member);
    assert_eq!(member_info.tier, MembershipTier::Premium);

    // Check updated voting power
    let voting_power = client.get_user_voting_power(&member);
    assert_eq!(voting_power, 20000);
}

#[test]
fn test_proposal_creation() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);
    let proposer = Address::generate(&env);
    let target_contract = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &100,
        &604800,
        &1000,
        &10,
        &86400,
        &50,
    );

    let token_client = RewardTokenClient::new(&env, &token_id);
    let admin = Address::generate(&env);
    token_client.initialize(
        &admin,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
        &6,
    );
    token_client.mint(&admin, &proposer, &5000);
    client.join_dao(&proposer, &5000);

    // Create proposal action
    let action = crate::types::ProposalActionInput {
        contract_id: target_contract.clone(),
        function_name: Symbol::new(&env, "some_function"),
        args: Vec::new(&env),
    };

    // Create proposal
    let proposal_id = client.propose(
        &proposer,
        &String::from_str(&env, "Test Proposal"),
        &String::from_str(&env, "Test Description"),
        &Some(action),
        &(ProposalCategory::PuzzleCuration as u32),
    );

    assert!(proposal_id > 0);

    // Check proposal info
    let proposal = client.get_proposal_info(&proposal_id);
    assert_eq!(proposal.proposer, proposer);
    assert_eq!(proposal.category, ProposalCategory::PuzzleCuration as u32);
    assert_eq!(proposal.status, crate::types::ProposalStatus::Pending);
}

#[test]
fn test_voting_process() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);
    let proposer = Address::generate(&env);
    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let target_contract = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &0, // No voting delay for testing
        &604800,
        &1000,
        &10,
        &86400,
        &50,
    );

    let token_client = RewardTokenClient::new(&env, &token_id);
    let admin = Address::generate(&env);
    token_client.initialize(
        &admin,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
        &6,
    );
    token_client.mint(&admin, &proposer, &5000);
    token_client.mint(&admin, &voter1, &3000);
    token_client.mint(&admin, &voter2, &2000);

    client.join_dao(&proposer, &5000);
    client.join_dao(&voter1, &3000);
    client.join_dao(&voter2, &2000);

    // Create proposal
    let action = crate::types::ProposalActionInput {
        contract_id: target_contract.clone(),
        function_name: Symbol::new(&env, "some_function"),
        args: Vec::new(&env),
    };

    let proposal_id = client.propose(
        &proposer,
        &String::from_str(&env, "Test Proposal"),
        &String::from_str(&env, "Test Description"),
        &Some(action),
        &(ProposalCategory::PuzzleCuration as u32),
    );

    // Advance time to start voting
    env.ledger().with_mut(|li| {
        li.timestamp += 1;
    });

    // Vote
    client.vote(&voter1, &proposal_id, &VoteType::For);
    client.vote(&voter2, &proposal_id, &VoteType::Against);

    // Check proposal state
    let proposal = client.get_proposal_info(&proposal_id);
    assert_eq!(proposal.for_votes, 3000);
    assert_eq!(proposal.against_votes, 2000);
    assert_eq!(proposal.status, crate::types::ProposalStatus::Active);
}

#[test]
fn test_vote_delegation() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);
    let delegator = Address::generate(&env);
    let delegatee = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &100,
        &604800,
        &1000,
        &10,
        &86400,
        &50,
    );

    let token_client = RewardTokenClient::new(&env, &token_id);
    let admin = Address::generate(&env);
    token_client.initialize(
        &admin,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
        &6,
    );
    token_client.mint(&admin, &delegator, &5000);
    client.join_dao(&delegator, &5000);

    // Check initial voting power
    let initial_power = client.get_user_voting_power(&delegator);
    assert_eq!(initial_power, 5000);

    let delegatee_power = client.get_user_voting_power(&delegatee);
    assert_eq!(delegatee_power, 0);

    // Delegate voting power
    client.delegate(&delegator, &delegatee);

    // Check updated voting power
    let delegator_power = client.get_user_voting_power(&delegator);
    assert_eq!(delegator_power, 0);

    let delegatee_power = client.get_user_voting_power(&delegatee);
    assert_eq!(delegatee_power, 5000);
}

#[test]
fn test_emergency_proposal() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);
    let proposer = Address::generate(&env);
    let target_contract = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &0,    // No voting delay for emergency
        &3600, // Shorter voting period for emergency
        &1000,
        &10,
        &86400,
        &50, // Higher quorum for emergency
    );

    let token_client = RewardTokenClient::new(&env, &token_id);
    let admin = Address::generate(&env);
    token_client.initialize(
        &admin,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
        &6,
    );
    token_client.mint(&admin, &proposer, &1000);
    client.join_dao(&proposer, &1000);

    // Create emergency proposal (lower threshold)
    let action = crate::types::ProposalActionInput {
        contract_id: target_contract.clone(),
        function_name: Symbol::new(&env, "emergency_function"),
        args: Vec::new(&env),
    };

    let proposal_id = client.propose(
        &proposer,
        &String::from_str(&env, "Emergency Proposal"),
        &String::from_str(&env, "Emergency Description"),
        &Some(action),
        &(ProposalCategory::Emergency as u32),
    );

    let proposal = client.get_proposal_info(&proposal_id);
    assert_eq!(proposal.category, ProposalCategory::Emergency as u32);
}

#[test]
fn test_treasury_management() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);
    let _recipient = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &100,
        &604800,
        &1000,
        &10,
        &86400,
        &50,
    );

    let token_client = RewardTokenClient::new(&env, &token_id);
    let admin = Address::generate(&env);
    token_client.initialize(
        &admin,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
        &6,
    );

    // Fund treasury
    token_client.mint(&admin, &treasury_address, &10000);

    // This would normally be called through a governance proposal
    // For testing, we'll check the treasury info
    let treasury_info = client.get_treasury_balance();
    assert_eq!(treasury_info.total_balance, 0); // Not updated until funds are transferred to contract
}

// ───────────── MULTISIG TESTS (ADR-0013) ─────────────

#[test]
fn test_multisig_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);
    let admin = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &100,
        &604800,
        &1000,
        &10,
        &86400,
        &50,
    );

    // Initialize multisig with threshold=2, TTL=3600s
    client.initialize_multisig(&admin, &2, &3600);

    let cfg = client.get_multisig_info();
    assert_eq!(cfg.threshold, 2);
    assert_eq!(cfg.action_ttl, 3600);
}

#[test]
fn test_single_admin_cannot_allocate_treasury() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &100,
        &604800,
        &1000,
        &10,
        &86400,
        &50,
    );

    // Direct call to allocate_treasury_funds should panic
    let result = client.try_allocate_treasury_funds(&1000, &recipient);
    assert!(result.is_err());
}

#[test]
fn test_propose_admin_action() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);
    let council1 = Address::generate(&env);
    let council2 = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &100,
        &604800,
        &1000,
        &10,
        &86400,
        &50,
    );

    // Setup members
    let token_client = RewardTokenClient::new(&env, &token_id);
    token_client.initialize(&Address::generate(&env), &String::from_str(&env, "T"), &String::from_str(&env, "T"), &6);
    token_client.mint(&Address::generate(&env), &council1, &100000);
    token_client.mint(&Address::generate(&env), &council2, &100000);
    client.join_dao(&council1, &100000);
    client.join_dao(&council2, &100000);

    // Initialize multisig with threshold=2
    client.initialize_multisig(&council1, &2, &3600);

    // Propose action
    let action_id = client.propose_admin_action(
        &council1,
        &String::from_str(&env, "Allocate 500 tokens"),
    );
    assert_eq!(action_id, 1);

    let action = client.get_admin_action_info(&action_id);
    assert_eq!(action.proposer, council1);
    assert_eq!(action.status, crate::types::AdminActionStatus::Pending);
    assert_eq!(action.signers.len(), 1); // proposer auto-signed
}

#[test]
fn test_sign_admin_action_and_approve() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);
    let council1 = Address::generate(&env);
    let council2 = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &100,
        &604800,
        &1000,
        &10,
        &86400,
        &50,
    );

    let token_client = RewardTokenClient::new(&env, &token_id);
    let admin_addr = Address::generate(&env);
    token_client.initialize(&admin_addr, &String::from_str(&env, "T"), &String::from_str(&env, "T"), &6);
    token_client.mint(&admin_addr, &council1, &100000);
    token_client.mint(&admin_addr, &council2, &100000);
    client.join_dao(&council1, &100000);
    client.join_dao(&council2, &100000);

    client.initialize_multisig(&council1, &2, &3600);

    let action_id = client.propose_admin_action(
        &council1,
        &String::from_str(&env, "Spend 500"),
    );

    // Before second signature: still Pending
    let action = client.get_admin_action_info(&action_id);
    assert_eq!(action.status, crate::types::AdminActionStatus::Pending);

    // Second council member signs
    client.sign_admin_action(&council2, &action_id);

    // After meeting threshold: Approved
    let action = client.get_admin_action_info(&action_id);
    assert_eq!(action.status, crate::types::AdminActionStatus::Approved);
    assert_eq!(action.signers.len(), 2);
}

#[test]
fn test_non_council_cannot_propose() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);
    let council = Address::generate(&env);
    let basic_member = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &100,
        &604800,
        &1000,
        &10,
        &86400,
        &50,
    );

    let token_client = RewardTokenClient::new(&env, &token_id);
    let admin_addr = Address::generate(&env);
    token_client.initialize(&admin_addr, &String::from_str(&env, "T"), &String::from_str(&env, "T"), &6);
    token_client.mint(&admin_addr, &council, &100000);
    token_client.mint(&admin_addr, &basic_member, &5000);
    client.join_dao(&council, &100000);
    client.join_dao(&basic_member, &5000); // Basic tier

    client.initialize_multisig(&council, &2, &3600);

    // Basic member tries to propose
    let result = client.try_propose_admin_action(
        &basic_member,
        &String::from_str(&env, "Unauthorized"),
    );
    assert!(result.is_err());
}

#[test]
fn test_execute_approved_action() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);
    let council1 = Address::generate(&env);
    let council2 = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &100,
        &604800,
        &1000,
        &10,
        &86400,
        &50,
    );

    let token_client = RewardTokenClient::new(&env, &token_id);
    let admin_addr = Address::generate(&env);
    token_client.initialize(&admin_addr, &String::from_str(&env, "T"), &String::from_str(&env, "T"), &6);
    token_client.mint(&admin_addr, &council1, &100000);
    token_client.mint(&admin_addr, &council2, &100000);
    client.join_dao(&council1, &100000);
    client.join_dao(&council2, &100000);

    client.initialize_multisig(&council1, &2, &3600);

    let action_id = client.propose_admin_action(
        &council1,
        &String::from_str(&env, "Execute me"),
    );
    client.sign_admin_action(&council2, &action_id);

    // Execute
    client.execute_admin_action(&council1, &action_id);

    let action = client.get_admin_action_info(&action_id);
    assert_eq!(action.status, crate::types::AdminActionStatus::Executed);
    assert!(action.executed_at.is_some());
}

#[test]
fn test_cannot_execute_non_approved_action() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PuzzleDaoContract);
    let client = PuzzleDaoContractClient::new(&env, &contract_id);

    let token_id = env.register_contract(None, RewardToken);
    let treasury_address = Address::generate(&env);
    let council = Address::generate(&env);
    let council2 = Address::generate(&env);

    client.initialize(
        &token_id,
        &treasury_address,
        &100,
        &604800,
        &1000,
        &10,
        &86400,
        &50,
    );

    let token_client = RewardTokenClient::new(&env, &token_id);
    let admin_addr = Address::generate(&env);
    token_client.initialize(&admin_addr, &String::from_str(&env, "T"), &String::from_str(&env, "T"), &6);
    token_client.mint(&admin_addr, &council, &100000);
    client.join_dao(&council, &100000);
    client.join_dao(&council2, &100000);

    client.initialize_multisig(&council, &2, &3600);

    let action_id = client.propose_admin_action(
        &council,
        &String::from_str(&env, "Not approved yet"),
    );

    // Should fail because action is still Pending (only 1 of 2 signatures)
    let result = client.try_execute_admin_action(&council, &action_id);
    assert!(result.is_err());
}
