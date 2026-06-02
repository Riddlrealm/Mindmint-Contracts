#![cfg(test)]

use super::*;
use soroban_sdk::{Env, Address};

#[test]
fn test_contribute() {
    let env = Env::default();

    let admin = Address::random(&env);
    let oracle = Address::random(&env);

    ProgressiveJackpot::init(env.clone(), admin.clone(), oracle.clone());

    oracle.require_auth();

    ProgressiveJackpot::contribute(env.clone(), 100);

    let jackpot = ProgressiveJackpot::get_jackpot(env.clone());

    assert_eq!(jackpot.balance, 100);
}

#[test]
fn test_claim_success() {
    let env = Env::default();

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    let player = Address::random(&env);

    ProgressiveJackpot::init(env.clone(), admin.clone(), oracle.clone());

    oracle.require_auth();
    ProgressiveJackpot::contribute(env.clone(), 500);

    admin.require_auth();
    ProgressiveJackpot::set_jackpot_puzzle(env.clone(), 1, env.ledger().timestamp() + 1000);

    player.require_auth();
    oracle.require_auth();

    ProgressiveJackpot::claim_jackpot(env.clone(), 1, player.clone());

    let jackpot = ProgressiveJackpot::get_jackpot(env.clone());

    assert_eq!(jackpot.balance, 0);
}

#[test]
#[should_panic]
fn test_invalid_claim() {
    let env = Env::default();

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    let player = Address::random(&env);

    ProgressiveJackpot::init(env.clone(), admin.clone(), oracle.clone());

    player.require_auth();
    oracle.require_auth();

    ProgressiveJackpot::claim_jackpot(env.clone(), 999, player.clone());
}

#[test]
fn test_rollover() {
    let env = Env::default();

    let admin = Address::random(&env);
    let oracle = Address::random(&env);

    ProgressiveJackpot::init(env.clone(), admin.clone(), oracle.clone());

    oracle.require_auth();
    ProgressiveJackpot::contribute(env.clone(), 200);

    admin.require_auth();
    ProgressiveJackpot::set_jackpot_puzzle(env.clone(), 1, 1); // expired

    ProgressiveJackpot::rollover(env.clone());

    let jackpot = ProgressiveJackpot::get_jackpot(env.clone());

    assert_eq!(jackpot.cycle_id, 2);
    assert_eq!(jackpot.balance, 200);
}

#[test]
fn test_double_claim_rejection() {
    let env = Env::default();

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    let player = Address::random(&env);

    ProgressiveJackpot::init(env.clone(), admin.clone(), oracle.clone());

    oracle.require_auth();
    ProgressiveJackpot::contribute(env.clone(), 500);

    admin.require_auth();
    ProgressiveJackpot::set_jackpot_puzzle(env.clone(), 1, env.ledger().timestamp() + 1000);

    player.require_auth();
    oracle.require_auth();

    // Verify player initially has not claimed
    assert!(!ProgressiveJackpot::has_claimed(env.clone(), player.clone()));
    assert_eq!(ProgressiveJackpot::get_claimed_amount(env.clone(), player.clone()), 0);

    // First claim succeeds
    ProgressiveJackpot::claim_jackpot(env.clone(), 1, player.clone());

    // Verify player is now recorded as having claimed the jackpot amount
    assert!(ProgressiveJackpot::has_claimed(env.clone(), player.clone()));
    assert_eq!(ProgressiveJackpot::get_claimed_amount(env.clone(), player.clone()), 500);

    // Second claim attempt should fail
    player.require_auth();
    oracle.require_auth();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ProgressiveJackpot::claim_jackpot(env.clone(), 1, player.clone());
    }));
    assert!(result.is_err());
}

#[test]
fn test_separate_player_claims() {
    let env = Env::default();

    let admin = Address::random(&env);
    let oracle = Address::random(&env);
    let player_1 = Address::random(&env);
    let player_2 = Address::random(&env);

    ProgressiveJackpot::init(env.clone(), admin.clone(), oracle.clone());

    // Cycle 1: Player 1 wins
    oracle.require_auth();
    ProgressiveJackpot::contribute(env.clone(), 500);

    admin.require_auth();
    ProgressiveJackpot::set_jackpot_puzzle(env.clone(), 1, env.ledger().timestamp() + 1000);

    player_1.require_auth();
    oracle.require_auth();
    ProgressiveJackpot::claim_jackpot(env.clone(), 1, player_1.clone());

    // Rollover to Cycle 2
    env.ledger().with_mut(|li| li.timestamp += 1001); // expire cycle 1
    ProgressiveJackpot::rollover(env.clone());

    // Cycle 2: Player 2 wins
    oracle.require_auth();
    ProgressiveJackpot::contribute(env.clone(), 300);

    admin.require_auth();
    ProgressiveJackpot::set_jackpot_puzzle(env.clone(), 2, env.ledger().timestamp() + 1000);

    player_2.require_auth();
    oracle.require_auth();
    ProgressiveJackpot::claim_jackpot(env.clone(), 2, player_2.clone());

    // Verify distinct tracking
    assert!(ProgressiveJackpot::has_claimed(env.clone(), player_1.clone()));
    assert_eq!(ProgressiveJackpot::get_claimed_amount(env.clone(), player_1.clone()), 500);

    assert!(ProgressiveJackpot::has_claimed(env.clone(), player_2.clone()));
    assert_eq!(ProgressiveJackpot::get_claimed_amount(env.clone(), player_2.clone()), 800); // 500 roll-over + 300 contribution
}