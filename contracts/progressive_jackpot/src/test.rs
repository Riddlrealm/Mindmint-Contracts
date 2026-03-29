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