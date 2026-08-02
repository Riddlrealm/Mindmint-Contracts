use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::testutils::Ledger as _;

#[test]
fn test_verification_flow() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleVerification);
    let client = PuzzleVerificationClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let player = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    env.ledger().set_timestamp(1_000);

    let preimage = Bytes::from_array(&env, &[7u8; 5]);
    let hash: BytesN<32> = env.crypto().sha256(&preimage).into();
    let now = env.ledger().timestamp();

    client.set_puzzle(&1, &hash, &(now - 1), &(now + 1000), &2, &50);

    let wrong = Bytes::from_array(&env, &[8u8; 5]);
    assert_eq!(client.verify_solution(&player, &1, &wrong), false);

    assert_eq!(client.verify_solution(&player, &1, &preimage), true);
    assert_eq!(client.is_completed(&player, &1), true);
    assert_eq!(client.rewards_of(&player), 100);
}

#[test]
#[should_panic(expected = "puzzle not active")]
fn test_expiration_enforced() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleVerification);
    let client = PuzzleVerificationClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let player = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    env.ledger().set_timestamp(1_000);

    let preimage = Bytes::from_array(&env, &[1u8; 3]);
    let hash: BytesN<32> = env.crypto().sha256(&preimage).into();
    let now = env.ledger().timestamp();

    client.set_puzzle(&42, &hash, &(now - 100), &(now - 50), &1, &10);

    let _ = client.verify_solution(&player, &42, &preimage);
}

/// Regression test for Issue #15: reward arithmetic must not silently wrap.
/// `i128::MAX` reward points multiplied by `u32::MAX` difficulty overflows
/// `i128`, so `verify_solution` must abort with `Error::RewardOverflow`
/// (manifested here as a panic) rather than corrupting ledger state.
#[test]
#[should_panic]
fn test_reward_overflow_panics() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleVerification);
    let client = PuzzleVerificationClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let player = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    env.ledger().set_timestamp(1_000);

    let preimage = Bytes::from_array(&env, &[3u8; 4]);
    let hash: BytesN<32> = env.crypto().sha256(&preimage).into();
    let now = env.ledger().timestamp();

    // MAX reward points × MAX difficulty → checked_mul overflows i128.
    client.set_puzzle(
        &7,
        &hash,
        &(now - 1),
        &(now + 1000),
        &u32::MAX,
        &i128::MAX,
    );

    let _ = client.verify_solution(&player, &7, &preimage);
}

/// Sanity check that a large-but-safe reward still accrues correctly.
#[test]
fn test_large_reward_accrues() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PuzzleVerification);
    let client = PuzzleVerificationClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let player = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    env.ledger().set_timestamp(1_000);

    let preimage = Bytes::from_array(&env, &[5u8; 6]);
    let hash: BytesN<32> = env.crypto().sha256(&preimage).into();
    let now = env.ledger().timestamp();

    // 1_000_000 reward points × difficulty 3 = 3_000_000 (no overflow).
    client.set_puzzle(&9, &hash, &(now - 1), &(now + 1000), &3, &1_000_000);

    assert_eq!(client.verify_solution(&player, &9, &preimage), true);
    assert_eq!(client.rewards_of(&player), 3_000_000);
}
