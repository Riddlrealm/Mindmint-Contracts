#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, IntoVal,
};

fn create_token_contract<'a>(env: &Env, admin: &Address) -> (Address, TokenClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let address = sac.address();
    (address.clone(), TokenClient::new(env, &address))
}

fn setup_contract(
    env: &Env,
) -> (
    SocialWagerContractClient,
    Address,
    Address,
    Address,
    Address,
    TokenClient,
    StellarAssetClient,
) {
    env.mock_all_auths();

    let admin = Address::generate(env);
    let challenger = Address::generate(env);
    let opponent = Address::generate(env);
    let oracle = Address::generate(env);
    let token_admin = Address::generate(env);

    let (token, token_client) = create_token_contract(env, &token_admin);
    let asset_admin = StellarAssetClient::new(env, &token);

    asset_admin.mint(&challenger, &1_000_000);
    asset_admin.mint(&opponent, &1_000_000);

    let contract_id = env.register_contract(None, SocialWagerContract);
    let client = SocialWagerContractClient::new(env, &contract_id);
    client.initialize(&admin, &token, &oracle, &None);

    (
        client,
        admin,
        challenger,
        opponent,
        oracle,
        token_client,
        asset_admin,
    )
}

#[test]
fn test_create_wager() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, challenger, opponent, _, token_client, _) = setup_contract(&env);
    let challenger_before = token_client.balance(&challenger);

    env.set_invoker(challenger.clone());
    let wager_id = client.create_wager(&opponent, &42u32, &100_000i128, &WagerType::Speed);

    let wager = client.get_wager(&wager_id);
    assert_eq!(wager.wager_id, 1);
    assert_eq!(wager.challenger, challenger);
    assert_eq!(wager.opponent, opponent);
    assert_eq!(wager.puzzle_id, 42);
    assert_eq!(wager.stake_amount, 100_000);
    assert_eq!(wager.wager_type, WagerType::Speed);
    assert_eq!(wager.status, WagerStatus::Pending);
    assert_eq!(token_client.balance(&wager.challenger), challenger_before - 100_000);
}

#[test]
fn test_accept_wager_escrows_both_sides() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, challenger, opponent, _, token_client, _) = setup_contract(&env);

    env.set_invoker(challenger.clone());
    let wager_id = client.create_wager(&opponent, &7u32, &150_000i128, &WagerType::Score);

    let contract_balance_before = token_client.balance(&client.address);

    env.set_invoker(opponent.clone());
    let wager = client.accept_wager(&wager_id);

    assert_eq!(wager.status, WagerStatus::Active);
    assert_eq!(token_client.balance(&client.address), contract_balance_before + 150_000);
    assert_eq!(token_client.balance(&challenger), 850_000);
    assert_eq!(token_client.balance(&opponent), 850_000);
}

#[test]
fn test_decline_wager_refunds_challenger() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, challenger, opponent, _, token_client, _) = setup_contract(&env);

    env.set_invoker(challenger.clone());
    let wager_id = client.create_wager(&opponent, &9u32, &100_000i128, &WagerType::Speed);

    env.set_invoker(opponent);
    let wager = client.decline_wager(&wager_id);

    assert_eq!(wager.status, WagerStatus::Declined);
    assert_eq!(token_client.balance(&challenger), 1_000_000);
    assert_eq!(token_client.balance(&client.address), 0);
}

#[test]
fn test_only_oracle_can_submit_result() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, challenger, opponent, oracle, _, _) = setup_contract(&env);

    env.set_invoker(challenger.clone());
    let wager_id = client.create_wager(&opponent, &1u32, &75_000i128, &WagerType::Score);

    env.set_invoker(opponent.clone());
    client.accept_wager(&wager_id);

    let non_oracle = Address::generate(&env);

    let result = client.try_submit_result(&non_oracle, &wager_id, &challenger);
    assert_eq!(result.err(), Some(Ok(SocialWagerError::Unauthorized)));

    let wager = client.submit_result(&oracle, &wager_id, &challenger);
    assert_eq!(wager.status, WagerStatus::ResultSubmitted);
    assert_eq!(wager.winner, Some(challenger));
}

#[test]
fn test_claim_winnings_with_default_fee() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, challenger, opponent, oracle, token_client, _) = setup_contract(&env);

    env.set_invoker(challenger.clone());
    let wager_id = client.create_wager(&opponent, &3u32, &100_000i128, &WagerType::Speed);

    env.set_invoker(opponent.clone());
    client.accept_wager(&wager_id);

    client.submit_result(&oracle, &wager_id, &challenger);

    let challenger_before = token_client.balance(&challenger);
    let admin_before = token_client.balance(&admin);

    env.set_invoker(challenger.clone());
    let wager = client.claim_winnings(&wager_id);

    assert_eq!(wager.status, WagerStatus::Claimed);
    assert_eq!(token_client.balance(&challenger), challenger_before + 196_000);
    assert_eq!(token_client.balance(&admin), admin_before + 4_000);
    assert_eq!(token_client.balance(&client.address), 0);
}

#[test]
fn test_auto_cancel_after_24_hours() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    let (client, _, challenger, opponent, _, token_client, _) = setup_contract(&env);

    env.set_invoker(challenger.clone());
    let wager_id = client.create_wager(&opponent, &12u32, &125_000i128, &WagerType::Score);

    env.ledger()
        .set_timestamp(1_000 + ACCEPTANCE_WINDOW_SECS + 1);

    let wager = client.get_wager(&wager_id);
    assert_eq!(wager.status, WagerStatus::Cancelled);
    assert_eq!(token_client.balance(&challenger), 1_000_000);
    assert_eq!(token_client.balance(&client.address), 0);
}

#[test]
fn test_fee_deduction_after_fee_update() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, challenger, opponent, oracle, token_client, _) = setup_contract(&env);
    client.set_fee_bps(&admin, &500u32);

    env.set_invoker(challenger.clone());
    let wager_id = client.create_wager(&opponent, &99u32, &200_000i128, &WagerType::Score);

    env.set_invoker(opponent.clone());
    client.accept_wager(&wager_id);
    client.submit_result(&oracle, &wager_id, &opponent);

    let opponent_before = token_client.balance(&opponent);
    let admin_before = token_client.balance(&admin);

    env.set_invoker(opponent.clone());
    client.claim_winnings(&wager_id);

    assert_eq!(token_client.balance(&opponent), opponent_before + 380_000);
    assert_eq!(token_client.balance(&admin), admin_before + 20_000);
}

#[test]
fn test_events_emitted_for_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, challenger, opponent, oracle, _, _) = setup_contract(&env);

    env.set_invoker(challenger.clone());
    let wager_id = client.create_wager(&opponent, &5u32, &50_000i128, &WagerType::Speed);

    env.set_invoker(opponent.clone());
    client.accept_wager(&wager_id);
    client.submit_result(&oracle, &wager_id, &challenger);

    env.set_invoker(challenger);
    client.claim_winnings(&wager_id);

    let events = env.events().all();
    assert!(events.len() >= 4);

    let create_event = events.get(0).unwrap();
    assert_eq!(
        create_event.0,
        (EVENT_WAGER, EVENT_CREATE).into_val(&env)
    );
}
