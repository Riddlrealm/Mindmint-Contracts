use soroban_sdk::{Address, Env, Symbol, Vec, symbol_short};
use crate::{PuzzleCoCreation, PuzzleCoCreationClient, CreatorShare, CoCreationStatus};

#[test]
fn test_initialize() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let retrieved_oracle = client.get_oracle();
    assert_eq!(retrieved_oracle, oracle);
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_initialize_twice() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);
    client.initialize(&oracle);
}

#[test]
fn test_initiate() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);
    let creator2 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 7000,
    });
    creators.push_back(CreatorShare {
        address: creator2.clone(),
        share_bps: 3000,
    });

    let puzzle_id = 123u64;
    let co_creation_id = client.initiate(&puzzle_id, &creators);

    let co_creation = client.get_co_creation(&co_creation_id);
    assert_eq!(co_creation.id, co_creation_id);
    assert_eq!(co_creation.puzzle_id, puzzle_id);
    assert_eq!(co_creation.status, CoCreationStatus::PendingSignatures);
    assert_eq!(co_creation.creators.len(), 2);
}

#[test]
#[should_panic(expected = "Shares must sum to exactly 10000 basis points")]
fn test_initiate_invalid_share_sum() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);
    let creator2 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1,
        share_bps: 5000,
    });
    creators.push_back(CreatorShare {
        address: creator2,
        share_bps: 4000, // Sum is 9000, not 10000
    });

    client.initiate(&123u64, &creators);
}

#[test]
#[should_panic(expected = "Invalid share: must be 1-10000 basis points")]
fn test_initiate_invalid_share_value() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1,
        share_bps: 0, // Invalid: cannot be 0
    });

    client.initiate(&123u64, &creators);
}

#[test]
#[should_panic(expected = "At least one creator required")]
fn test_initiate_no_creators() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creators = Vec::new(&env);
    client.initiate(&123u64, &creators);
}

#[test]
#[should_panic(expected = "Duplicate creator address")]
fn test_initiate_duplicate_creator() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 5000,
    });
    creators.push_back(CreatorShare {
        address: creator1.clone(), // Duplicate
        share_bps: 5000,
    });

    client.initiate(&123u64, &creators);
}

#[test]
fn test_sign() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);
    let creator2 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 7000,
    });
    creators.push_back(CreatorShare {
        address: creator2.clone(),
        share_bps: 3000,
    });

    let co_creation_id = client.initiate(&123u64, &creators);

    // Creator 2 signs
    client.sign(&co_creation_id, &creator2);

    let co_creation = client.get_co_creation(&co_creation_id);
    assert_eq!(co_creation.signatures.len(), 2);
    assert!(client.has_signed(&co_creation_id, &creator2));
}

#[test]
#[should_panic(expected = "Not a creator")]
fn test_sign_not_creator() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);
    let creator2 = Address::generate(&env);
    let non_creator = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1,
        share_bps: 10000,
    });

    let co_creation_id = client.initiate(&123u64, &creators);

    // Non-creator tries to sign
    client.sign(&co_creation_id, &non_creator);
}

#[test]
#[should_panic(expected = "Already signed")]
fn test_sign_already_signed() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 10000,
    });

    let co_creation_id = client.initiate(&123u64, &creators);

    // Try to sign again
    client.sign(&co_creation_id, &creator1);
}

#[test]
fn test_publish_all_signed() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);
    let creator2 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 7000,
    });
    creators.push_back(CreatorShare {
        address: creator2.clone(),
        share_bps: 3000,
    });

    let co_creation_id = client.initiate(&123u64, &creators);

    // Creator 2 signs
    client.sign(&co_creation_id, &creator2);

    // Publish
    client.publish(&co_creation_id);

    let co_creation = client.get_co_creation(&co_creation_id);
    assert_eq!(co_creation.status, CoCreationStatus::Published);
    assert!(co_creation.published_at.is_some());
}

#[test]
#[should_panic(expected = "Not all creators have signed")]
fn test_publish_not_all_signed() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);
    let creator2 = Address::generate(&env);
    let creator3 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 5000,
    });
    creators.push_back(CreatorShare {
        address: creator2.clone(),
        share_bps: 3000,
    });
    creators.push_back(CreatorShare {
        address: creator3.clone(),
        share_bps: 2000,
    });

    let co_creation_id = client.initiate(&123u64, &creators);

    // Only creator 2 signs, creator 3 does not
    client.sign(&co_creation_id, &creator2);

    // Try to publish without all signatures
    client.publish(&co_creation_id);
}

#[test]
#[should_panic(expected = "Already published")]
fn test_publish_already_published() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 10000,
    });

    let co_creation_id = client.initiate(&123u64, &creators);

    // Publish
    client.publish(&co_creation_id);

    // Try to publish again
    client.publish(&co_creation_id);
}

#[test]
fn test_withdraw_signature() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);
    let creator2 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 7000,
    });
    creators.push_back(CreatorShare {
        address: creator2.clone(),
        share_bps: 3000,
    });

    let co_creation_id = client.initiate(&123u64, &creators);

    // Creator 2 signs
    client.sign(&co_creation_id, &creator2);

    // Creator 2 withdraws signature
    client.withdraw_signature(&co_creation_id, &creator2);

    let co_creation = client.get_co_creation(&co_creation_id);
    assert_eq!(co_creation.signatures.len(), 1);
    assert!(!client.has_signed(&co_creation_id, &creator2));
}

#[test]
fn test_withdraw_signature_reverts_to_draft() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);
    let creator2 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 7000,
    });
    creators.push_back(CreatorShare {
        address: creator2.clone(),
        share_bps: 3000,
    });

    let co_creation_id = client.initiate(&123u64, &creators);

    // Creator 2 signs
    client.sign(&co_creation_id, &creator2);

    // Creator 1 withdraws signature (lead creator)
    client.withdraw_signature(&co_creation_id, &creator1);

    let co_creation = client.get_co_creation(&co_creation_id);
    assert_eq!(co_creation.status, CoCreationStatus::Draft);
    assert_eq!(co_creation.signatures.len(), 0);
}

#[test]
#[should_panic(expected = "Cannot withdraw from published co-creation")]
fn test_withdraw_signature_published() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 10000,
    });

    let co_creation_id = client.initiate(&123u64, &creators);

    // Publish
    client.publish(&co_creation_id);

    // Try to withdraw signature
    client.withdraw_signature(&co_creation_id, &creator1);
}

#[test]
#[should_panic(expected = "Not signed")]
fn test_withdraw_signature_not_signed() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);
    let creator2 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 7000,
    });
    creators.push_back(CreatorShare {
        address: creator2.clone(),
        share_bps: 3000,
    });

    let co_creation_id = client.initiate(&123u64, &creators);

    // Creator 2 tries to withdraw without signing
    client.withdraw_signature(&co_creation_id, &creator2);
}

#[test]
fn test_distribute_royalty() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);
    let creator2 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 7000,
    });
    creators.push_back(CreatorShare {
        address: creator2.clone(),
        share_bps: 3000,
    });

    let co_creation_id = client.initiate(&123u64, &creators);
    client.sign(&co_creation_id, &creator2);
    client.publish(&co_creation_id);

    // Distribute royalty
    let total_amount = 1000i128;
    client.distribute_royalty(&co_creation_id, &total_amount);

    // Verify events were emitted (in real implementation, would check token transfers)
    // Creator 1 should get 700 (70%)
    // Creator 2 should get 300 (30%)
}

#[test]
#[should_panic(expected = "Co-creation not published")]
fn test_distribute_royalty_not_published() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 10000,
    });

    let co_creation_id = client.initiate(&123u64, &creators);

    // Try to distribute royalty before publishing
    client.distribute_royalty(&co_creation_id, &1000i128);
}

#[test]
#[should_panic(expected = "Invalid amount")]
fn test_distribute_royalty_invalid_amount() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 10000,
    });

    let co_creation_id = client.initiate(&123u64, &creators);
    client.publish(&co_creation_id);

    // Try to distribute invalid amount
    client.distribute_royalty(&co_creation_id, &0i128);
}

#[test]
fn test_get_co_creation() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    let creator1 = Address::generate(&env);

    let mut creators = Vec::new(&env);
    creators.push_back(CreatorShare {
        address: creator1.clone(),
        share_bps: 10000,
    });

    let puzzle_id = 123u64;
    let co_creation_id = client.initiate(&puzzle_id, &creators);

    let co_creation = client.get_co_creation(&co_creation_id);
    assert_eq!(co_creation.id, co_creation_id);
    assert_eq!(co_creation.puzzle_id, puzzle_id);
    assert_eq!(co_creation.creators.len(), 1);
    assert_eq!(co_creation.creators.get(0).unwrap().address, creator1);
    assert_eq!(co_creation.creators.get(0).unwrap().share_bps, 10000);
}

#[test]
#[should_panic(expected = "Co-creation not found")]
fn test_get_co_creation_not_found() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleCoCreation);
    let client = PuzzleCoCreationClient::new(&env, &contract_id);

    let oracle = Address::generate(&env);
    client.initialize(&oracle);

    client.get_co_creation(&999u64);
}
