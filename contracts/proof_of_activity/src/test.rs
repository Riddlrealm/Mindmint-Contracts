use soroban_sdk::{
    vec, Address, Env, Symbol,
};
use soroban_sdk::testutils::Address as TestAddress;
use crate::{
    ContractError, ProofOfActivityContract, ActivityType,
};

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, ProofOfActivityContract);

    let admin = TestAddress::generate(&env);
    
    // Test successful initialization
    ProofOfActivityContract::initialize(env.clone(), admin.clone());
    
    // Test duplicate initialization fails
    let result = ProofOfActivityContract::try_initialize(env.clone(), admin.clone());
    assert_eq!(result.unwrap_err(), ContractError::AlreadyInitialized);
}

#[test]
fn test_record_proof() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, ProofOfActivityContract);

    let admin = TestAddress::generate(&env);
    let oracle = TestAddress::generate(&env);
    let player = TestAddress::generate(&env);
    
    // Initialize contract
    ProofOfActivityContract::initialize(env.clone(), admin.clone());
    
    // Add oracle
    ProofOfActivityContract::add_oracle(env.clone(), admin.clone(), oracle.clone());
    
    // Record a proof
    let proof_id = ProofOfActivityContract::record_proof(
        env.clone(),
        oracle.clone(),
        player.clone(),
        ActivityType::PuzzleSolved,
        Symbol::from_str("puzzle_123"),
        100,
    );
    
    assert_eq!(proof_id, 1);
    
    // Verify the proof
    let proof = ProofOfActivityContract::get_proof(env.clone(), proof_id).unwrap();
    assert_eq!(proof.0, player);
    assert_eq!(proof.1, ActivityType::PuzzleSolved as u32);
    assert_eq!(proof.4, 100);
}

#[test]
fn test_unauthorized_oracle() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, ProofOfActivityContract);

    let admin = TestAddress::generate(&env);
    let unauthorized_oracle = TestAddress::generate(&env);
    let player = TestAddress::generate(&env);
    
    // Initialize contract
    ProofOfActivityContract::initialize(env.clone(), admin.clone());
    
    // Try to record proof with unauthorized oracle
    let result = ProofOfActivityContract::try_record_proof(
        env.clone(),
        unauthorized_oracle.clone(),
        player.clone(),
        ActivityType::PuzzleSolved,
        Symbol::from_str("puzzle_123"),
        100,
    );
    
    assert_eq!(result.unwrap_err(), ContractError::Unauthorized);
}

#[test]
fn test_score_aggregation() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, ProofOfActivityContract);

    let admin = TestAddress::generate(&env);
    let oracle = TestAddress::generate(&env);
    let player = TestAddress::generate(&env);
    
    // Initialize contract
    ProofOfActivityContract::initialize(env.clone(), admin.clone());
    
    // Add oracle
    ProofOfActivityContract::add_oracle(env.clone(), admin.clone(), oracle.clone());
    
    // Record multiple proofs
    ProofOfActivityContract::record_proof(
        env.clone(),
        oracle.clone(),
        player.clone(),
        ActivityType::PuzzleSolved,
        Symbol::from_str("puzzle_1"),
        100,
    );
    
    ProofOfActivityContract::record_proof(
        env.clone(),
        oracle.clone(),
        player.clone(),
        ActivityType::TournamentCompleted,
        Symbol::from_str("tournament_1"),
        200,
    );
    
    // Check total score
    let total_score = ProofOfActivityContract::get_activity_score(env.clone(), player.clone());
    assert_eq!(total_score, 300);
}
