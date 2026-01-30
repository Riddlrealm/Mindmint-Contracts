#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::Vec as SorobanVec;

fn setup() -> (Env, Address, Address, SeasonalEventContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, SeasonalEventContract);
    let client = SeasonalEventContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &None);

    (env, admin, user, client)
}

fn create_basic_event(
    env: &Env,
    client: &SeasonalEventContractClient<'_>,
    admin: &Address,
    start_time: u64,
    end_time: u64,
    bonus: u32,
) -> u64 {
    let mut puzzles = SorobanVec::new(env);
    puzzles.push_back(1);
    puzzles.push_back(2);

    client.create_event(
        admin,
        &String::from_str(env, "Winter Festival"),
        &start_time,
        &end_time,
        &1000i128,
        &bonus,
        &String::from_str(env, "winter_nft"),
        &puzzles,
    )
}

#[test]
fn event_activation_by_time() {
    let (env, admin, _user, client) = setup();

    env.ledger().set_timestamp(100);
    let event_id = create_basic_event(&env, &client, &admin, 200, 300, 10_000);

    assert!(!client.is_event_active(&event_id));

    env.ledger().set_timestamp(250);
    assert!(client.is_event_active(&event_id));

    env.ledger().set_timestamp(301);
    assert!(!client.is_event_active(&event_id));
}

#[test]
fn reward_claim_applies_bonus() {
    let (env, admin, user, client) = setup();

    env.ledger().set_timestamp(100);
    let event_id = create_basic_event(&env, &client, &admin, 100, 200, 15_000);

    client.record_puzzle_completion(&admin, &event_id, &user, &1u32, &25i128);

    let reward = client.claim_event_reward(&event_id, &user);
    assert_eq!(reward, 1500);
}

#[test]
#[should_panic(expected = "Event not active")]
fn reward_claim_fails_after_end() {
    let (env, admin, user, client) = setup();

    env.ledger().set_timestamp(100);
    let event_id = create_basic_event(&env, &client, &admin, 100, 200, 10_000);
    client.record_puzzle_completion(&admin, &event_id, &user, &1u32, &10i128);

    env.ledger().set_timestamp(250);
    client.claim_event_reward(&event_id, &user);
}

#[test]
fn mint_event_nft_once() {
    let (env, admin, user, client) = setup();

    env.ledger().set_timestamp(100);
    let event_id = create_basic_event(&env, &client, &admin, 100, 200, 10_000);

    client.record_puzzle_completion(&admin, &event_id, &user, &1u32, &10i128);
    client.claim_event_reward(&event_id, &user);

    let token_id = client.mint_event_nft(&event_id, &user);
    assert_eq!(token_id, 1);

    let nft = client.get_event_nft(&token_id).unwrap();
    assert_eq!(nft.event_id, event_id);
    assert_eq!(nft.owner, user);
}

#[test]
#[should_panic(expected = "Not a participant")]
fn reward_claim_requires_participation() {
    let (env, admin, user, client) = setup();

    env.ledger().set_timestamp(100);
    let event_id = create_basic_event(&env, &client, &admin, 100, 200, 10_000);

    client.claim_event_reward(&event_id, &user);
}
