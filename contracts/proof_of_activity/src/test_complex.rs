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
    let client = ProofOfActivityContractClient::new(&env, &contract_id);

    let admin = TestAddress::generate(&env);

    // Test successful initialization
    assert_eq!(client.initialize(&admin), Ok(()));

    // Test double initialization
    assert_eq!(
        client.initialize(&admin),
        Err(ContractError::AlreadyInitialized)
    );
}

#[test]
fn test_record_proof() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, ProofOfActivityContract);
    let client = ProofOfActivityContractClient::new(&env, &contract_id);

    let admin = TestAddress::generate(&env);
    let oracle = TestAddress::generate(&env);
    let player = TestAddress::generate(&env);

    // Initialize contract
    client.initialize(&admin).unwrap();

    // Add oracle
    client.add_oracle(&admin, &oracle).unwrap();

    // Test recording a proof
    let ref_id = symbol!("puzzle_123");
    let proof_id = client
        .record_proof(&oracle, &player, ActivityType::PuzzleSolved, ref_id, 100)
        .unwrap();

    assert_eq!(proof_id, 1);

    // Verify the proof was recorded correctly
    let proof = client.get_proof(&proof_id).unwrap();
    assert_eq!(proof.proof_id, proof_id);
    assert_eq!(proof.player, player);
    assert_eq!(proof.activity_type, ActivityType::PuzzleSolved);
    assert_eq!(proof.ref_id, ref_id);
    assert_eq!(proof.score, 100);
}

#[test]
fn test_unauthorized_oracle_rejection() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, ProofOfActivityContract);
    let client = ProofOfActivityContractClient::new(&env, &contract_id);

    let admin = TestAddress::generate(&env);
    let unauthorized_oracle = TestAddress::generate(&env);
    let player = TestAddress::generate(&env);

    // Initialize contract
    client.initialize(&admin).unwrap();

    // Test unauthorized oracle
    let ref_id = symbol!("puzzle_123");
    assert_eq!(
        client.record_proof(
            &unauthorized_oracle,
            &player,
            ActivityType::PuzzleSolved,
            ref_id,
            100
        ),
        Err(ContractError::Unauthorized)
    );
}

#[test]
fn test_score_aggregation() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, ProofOfActivityContract);
    let client = ProofOfActivityContractClient::new(&env, &contract_id);

    let admin = TestAddress::generate(&env);
    let oracle = TestAddress::generate(&env);
    let player = TestAddress::generate(&env);

    // Initialize contract
    client.initialize(&admin).unwrap();
    client.add_oracle(&admin, &oracle).unwrap();

    // Record multiple proofs with different scores
    client
        .record_proof(&oracle, &player, ActivityType::PuzzleSolved, symbol!("puzzle1"), 50)
        .unwrap();
    client
        .record_proof(&oracle, &player, ActivityType::TournamentCompleted, symbol!("tourn1"), 150)
        .unwrap();
    client
        .record_proof(&oracle, &player, ActivityType::WaveContributed, symbol!("wave1"), 75)
        .unwrap();

    // Check aggregated score
    let total_score = client.get_activity_score(&player);
    assert_eq!(total_score, 275); // 50 + 150 + 75
}

#[test]
fn test_append_only_enforcement() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, ProofOfActivityContract);
    let client = ProofOfActivityContractClient::new(&env, &contract_id);

    let admin = TestAddress::generate(&env);
    let oracle = TestAddress::generate(&env);
    let player = TestAddress::generate(&env);

    // Initialize contract
    client.initialize(&admin).unwrap();
    client.add_oracle(&admin, &oracle).unwrap();

    // Record initial proof
    let proof_id = client
        .record_proof(&oracle, &player, ActivityType::PuzzleSolved, symbol!("puzzle1"), 100)
        .unwrap();

    // Verify proof exists and has original data
    let proof = client.get_proof(&proof_id).unwrap();
    assert_eq!(proof.score, 100);

    // Record another proof (should create new proof, not modify existing)
    let proof_id2 = client
        .record_proof(&oracle, &player, ActivityType::PuzzleSolved, symbol!("puzzle2"), 200)
        .unwrap();

    assert_eq!(proof_id2, 2); // New proof ID

    // Verify original proof is unchanged
    let original_proof = client.get_proof(&proof_id).unwrap();
    assert_eq!(original_proof.score, 100);
    assert_eq!(original_proof.ref_id, symbol!("puzzle1"));
}

#[test]
fn test_oracle_management() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, ProofOfActivityContract);
    let client = ProofOfActivityContractClient::new(&env, &contract_id);

    let admin = TestAddress::generate(&env);
    let oracle1 = TestAddress::generate(&env);
    let oracle2 = TestAddress::generate(&env);
    let player = TestAddress::generate(&env);

    // Initialize contract
    client.initialize(&admin).unwrap();

    // Test admin is initially authorized
    assert!(client.is_authorized_oracle(&admin));

    // Add new oracle
    client.add_oracle(&admin, &oracle1).unwrap();
    assert!(client.is_authorized_oracle(&oracle1));

    // Test oracle can record proofs
    client
        .record_proof(&oracle1, &player, ActivityType::PuzzleSolved, symbol!("puzzle1"), 100)
        .unwrap();

    // Remove oracle
    client.remove_oracle(&admin, &oracle1).unwrap();
    assert!(!client.is_authorized_oracle(&oracle1));

    // Test removed oracle cannot record proofs
    assert_eq!(
        client.record_proof(&oracle1, &player, ActivityType::PuzzleSolved, symbol!("puzzle2"), 100),
        Err(ContractError::Unauthorized)
    );

    // Test unauthorized admin cannot manage oracles
    assert_eq!(
        client.add_oracle(&oracle2, &oracle2),
        Err(ContractError::Unauthorized)
    );
}

#[test]
fn test_pagination() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, ProofOfActivityContract);
    let client = ProofOfActivityContractClient::new(&env, &contract_id);

    let admin = TestAddress::generate(&env);
    let oracle = TestAddress::generate(&env);
    let player = TestAddress::generate(&env);

    // Initialize contract
    client.initialize(&admin).unwrap();
    client.add_oracle(&admin, &oracle).unwrap();

    // Record multiple proofs
    for i in 0..5 {
        client
            .record_proof(
                &oracle,
                &player,
                ActivityType::PuzzleSolved,
                symbol!(&format!("puzzle{}", i)),
                100 + i as u64,
            )
            .unwrap();
    }

    // Test pagination
    let proofs_page1 = client
        .get_player_proofs(&player, ActivityType::PuzzleSolved, 0, 3)
        .unwrap();
    assert_eq!(proofs_page1.len(), 3);

    let proofs_page2 = client
        .get_player_proofs(&player, ActivityType::PuzzleSolved, 3, 3)
        .unwrap();
    assert_eq!(proofs_page2.len(), 2);

    // Test invalid pagination
    assert_eq!(
        client.get_player_proofs(&player, ActivityType::PuzzleSolved, 0, 101),
        Err(ContractError::InvalidPagination)
    );
}

#[test]
fn test_different_activity_types() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, ProofOfActivityContract);
    let client = ProofOfActivityContractClient::new(&env, &contract_id);

    let admin = TestAddress::generate(&env);
    let oracle = TestAddress::generate(&env);
    let player = TestAddress::generate(&env);

    // Initialize contract
    client.initialize(&admin).unwrap();
    client.add_oracle(&admin, &oracle).unwrap();

    // Record proofs for different activity types
    client
        .record_proof(&oracle, &player, ActivityType::PuzzleSolved, symbol!("puzzle1"), 100)
        .unwrap();
    client
        .record_proof(&oracle, &player, ActivityType::TournamentCompleted, symbol!("tourn1"), 200)
        .unwrap();
    client
        .record_proof(&oracle, &player, ActivityType::WaveContributed, symbol!("wave1"), 150)
        .unwrap();

    // Verify proofs are correctly categorized
    let puzzle_proofs = client
        .get_player_proofs(&player, ActivityType::PuzzleSolved, 0, 10)
        .unwrap();
    assert_eq!(puzzle_proofs.len(), 1);
    assert_eq!(puzzle_proofs.get(0).unwrap().activity_type, ActivityType::PuzzleSolved);

    let tournament_proofs = client
        .get_player_proofs(&player, ActivityType::TournamentCompleted, 0, 10)
        .unwrap();
    assert_eq!(tournament_proofs.len(), 1);
    assert_eq!(tournament_proofs.get(0).unwrap().activity_type, ActivityType::TournamentCompleted);

    let wave_proofs = client
        .get_player_proofs(&player, ActivityType::WaveContributed, 0, 10)
        .unwrap();
    assert_eq!(wave_proofs.len(), 1);
    assert_eq!(wave_proofs.get(0).unwrap().activity_type, ActivityType::WaveContributed);
}

#[test]
fn test_proof_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, ProofOfActivityContract);
    let client = ProofOfActivityContractClient::new(&env, &contract_id);

    let admin = TestAddress::generate(&env);

    // Initialize contract
    client.initialize(&admin).unwrap();

    // Test getting non-existent proof
    assert_eq!(
        client.get_proof(&999),
        Err(ContractError::ProofNotFound)
    );
}
