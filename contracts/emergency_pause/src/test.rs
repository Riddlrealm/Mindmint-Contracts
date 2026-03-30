#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, String,
};

fn setup() -> (Env, EmergencyPauseContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, EmergencyPauseContract);
    let client = EmergencyPauseContractClient::new(&env, &contract_id);

    client.initialize(&admin);

    (env, client, admin)
}

fn advance_time(env: &Env, seconds: u64) {
    let mut ledger = env.ledger().get();
    ledger.timestamp += seconds;
    ledger.sequence_number += 1;
    env.ledger().set(ledger);
}

fn set_time(env: &Env, timestamp: u64) {
    let mut ledger = env.ledger().get();
    ledger.timestamp = timestamp;
    ledger.sequence_number += 1;
    env.ledger().set(ledger);
}

// ──────────────────────────────────────────────────────────
// INITIALIZATION
// ──────────────────────────────────────────────────────────

#[test]
fn test_initialize_success() {
    let (env, client, admin) = setup();

    // Admin should be in guardians
    let guardians = client.get_guardians();
    assert_eq!(guardians.len(), 1);
    assert_eq!(guardians.get(0).unwrap(), admin);

    // Default timelock is 24h
    assert_eq!(client.get_timelock(), 86_400);

    // Not paused
    assert_eq!(client.is_paused(), false);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_initialize_twice_fails() {
    let (env, client, admin) = setup();
    client.initialize(&admin);
}

// ──────────────────────────────────────────────────────────
// PAUSE
// ──────────────────────────────────────────────────────────

#[test]
fn test_pause_by_guardian() {
    let (env, client, admin) = setup();
    set_time(&env, 1000);

    let reason = String::from_str(&env, "Security incident detected");
    client.pause(&admin, &reason);

    assert_eq!(client.is_paused(), true);

    let state = client.get_pause_state();
    assert_eq!(state.paused, true);
    assert_eq!(state.paused_at, 1000);
    assert_eq!(state.paused_by, admin);
    assert_eq!(state.reason, reason);
    assert_eq!(state.unpause_after, 0);
}

#[test]
fn test_pause_by_added_guardian() {
    let (env, client, admin) = setup();

    let guardian2 = Address::generate(&env);
    client.add_guardian(&guardian2);

    let reason = String::from_str(&env, "Guardian 2 pausing");
    client.pause(&guardian2, &reason);

    assert_eq!(client.is_paused(), true);
    let state = client.get_pause_state();
    assert_eq!(state.paused_by, guardian2);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_pause_by_non_guardian_fails() {
    let (env, client, _admin) = setup();
    let stranger = Address::generate(&env);
    let reason = String::from_str(&env, "Unauthorized");
    client.pause(&stranger, &reason);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_pause_when_already_paused_fails() {
    let (env, client, admin) = setup();
    let reason = String::from_str(&env, "First pause");
    client.pause(&admin, &reason);

    let reason2 = String::from_str(&env, "Second pause");
    client.pause(&admin, &reason2);
}

// ──────────────────────────────────────────────────────────
// UNPAUSE FLOW
// ──────────────────────────────────────────────────────────

#[test]
fn test_request_unpause() {
    let (env, client, admin) = setup();
    set_time(&env, 1000);

    let reason = String::from_str(&env, "Incident");
    client.pause(&admin, &reason);

    set_time(&env, 2000);
    let unpause_after = client.request_unpause(&admin);

    // unpause_after = 2000 + 86400 (default timelock)
    assert_eq!(unpause_after, 2000 + 86_400);

    let state = client.get_pause_state();
    assert_eq!(state.unpause_after, unpause_after);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_request_unpause_when_not_paused_fails() {
    let (env, client, admin) = setup();
    client.request_unpause(&admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_request_unpause_by_non_guardian_fails() {
    let (env, client, admin) = setup();
    let reason = String::from_str(&env, "Incident");
    client.pause(&admin, &reason);

    let stranger = Address::generate(&env);
    client.request_unpause(&stranger);
}

#[test]
fn test_execute_unpause_after_timelock() {
    let (env, client, admin) = setup();
    set_time(&env, 1000);

    let reason = String::from_str(&env, "Incident");
    client.pause(&admin, &reason);

    set_time(&env, 2000);
    client.request_unpause(&admin);

    // Advance past timelock (2000 + 86400 = 88400)
    set_time(&env, 88_401);
    client.execute_unpause();

    assert_eq!(client.is_paused(), false);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_execute_unpause_before_timelock_fails() {
    let (env, client, admin) = setup();
    set_time(&env, 1000);

    let reason = String::from_str(&env, "Incident");
    client.pause(&admin, &reason);

    set_time(&env, 2000);
    client.request_unpause(&admin);

    // Try to unpause too early (before 2000 + 86400)
    set_time(&env, 50_000);
    client.execute_unpause();
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_execute_unpause_without_request_fails() {
    let (env, client, admin) = setup();
    let reason = String::from_str(&env, "Incident");
    client.pause(&admin, &reason);

    // Try to execute unpause without requesting first
    client.execute_unpause();
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_execute_unpause_when_not_paused_fails() {
    let (_env, client, _admin) = setup();
    client.execute_unpause();
}

#[test]
fn test_execute_unpause_at_exact_timelock_boundary() {
    let (env, client, admin) = setup();
    set_time(&env, 1000);

    let reason = String::from_str(&env, "Incident");
    client.pause(&admin, &reason);

    set_time(&env, 2000);
    let unpause_after = client.request_unpause(&admin);

    // Execute exactly at the timelock boundary
    set_time(&env, unpause_after);
    client.execute_unpause();

    assert_eq!(client.is_paused(), false);
}

// ──────────────────────────────────────────────────────────
// GUARDIAN MANAGEMENT
// ──────────────────────────────────────────────────────────

#[test]
fn test_add_guardian() {
    let (env, client, admin) = setup();
    let g2 = Address::generate(&env);
    let g3 = Address::generate(&env);

    client.add_guardian(&g2);
    client.add_guardian(&g3);

    let guardians = client.get_guardians();
    assert_eq!(guardians.len(), 3);
    assert_eq!(guardians.get(0).unwrap(), admin);
    assert_eq!(guardians.get(1).unwrap(), g2);
    assert_eq!(guardians.get(2).unwrap(), g3);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_add_duplicate_guardian_fails() {
    let (env, client, admin) = setup();
    // admin is already a guardian
    client.add_guardian(&admin);
}

#[test]
fn test_remove_guardian() {
    let (env, client, admin) = setup();
    let g2 = Address::generate(&env);
    client.add_guardian(&g2);

    client.remove_guardian(&g2);

    let guardians = client.get_guardians();
    assert_eq!(guardians.len(), 1);
    assert_eq!(guardians.get(0).unwrap(), admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_remove_nonexistent_guardian_fails() {
    let (env, client, _admin) = setup();
    let stranger = Address::generate(&env);
    client.remove_guardian(&stranger);
}

#[test]
fn test_removed_guardian_cannot_pause() {
    let (env, client, admin) = setup();
    let g2 = Address::generate(&env);
    client.add_guardian(&g2);
    client.remove_guardian(&g2);

    let reason = String::from_str(&env, "Trying to pause");
    // g2 is no longer a guardian — this should fail
    let result = client.try_pause(&g2, &reason);
    assert!(result.is_err());
}

// ──────────────────────────────────────────────────────────
// TIMELOCK CONFIGURATION
// ──────────────────────────────────────────────────────────

#[test]
fn test_set_timelock() {
    let (_env, client, _admin) = setup();

    // Change to 48 hours
    client.set_timelock(&172_800);
    assert_eq!(client.get_timelock(), 172_800);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_set_timelock_zero_fails() {
    let (_env, client, _admin) = setup();
    client.set_timelock(&0);
}

#[test]
fn test_custom_timelock_affects_unpause() {
    let (env, client, admin) = setup();

    // Set timelock to 1 hour
    client.set_timelock(&3600);

    set_time(&env, 1000);
    let reason = String::from_str(&env, "Incident");
    client.pause(&admin, &reason);

    set_time(&env, 2000);
    let unpause_after = client.request_unpause(&admin);
    assert_eq!(unpause_after, 2000 + 3600);

    // Should fail before 1h
    set_time(&env, 5000);
    let result = client.try_execute_unpause();
    assert!(result.is_err());

    // Should succeed after 1h
    set_time(&env, 5601);
    client.execute_unpause();
    assert_eq!(client.is_paused(), false);
}

// ──────────────────────────────────────────────────────────
// HISTORY
// ──────────────────────────────────────────────────────────

#[test]
fn test_pause_history_records_events() {
    let (env, client, admin) = setup();
    set_time(&env, 1000);

    let reason = String::from_str(&env, "Breach detected");
    client.pause(&admin, &reason);

    set_time(&env, 2000);
    client.request_unpause(&admin);

    set_time(&env, 90_000);
    client.execute_unpause();

    let history = client.get_pause_history();
    assert_eq!(history.len(), 3);

    // First event: paused
    let e0 = history.get(0).unwrap();
    assert_eq!(e0.action, Symbol::new(&env, "paused"));
    assert_eq!(e0.timestamp, 1000);

    // Second event: unpause requested
    let e1 = history.get(1).unwrap();
    assert_eq!(e1.action, Symbol::new(&env, "unpause_req"));
    assert_eq!(e1.timestamp, 2000);

    // Third event: unpaused
    let e2 = history.get(2).unwrap();
    assert_eq!(e2.action, Symbol::new(&env, "unpaused"));
    assert_eq!(e2.timestamp, 90_000);
}

#[test]
fn test_empty_history_initially() {
    let (_env, client, _admin) = setup();
    let history = client.get_pause_history();
    assert_eq!(history.len(), 0);
}

// ──────────────────────────────────────────────────────────
// FULL LIFECYCLE
// ──────────────────────────────────────────────────────────

#[test]
fn test_full_pause_unpause_cycle() {
    let (env, client, admin) = setup();

    // 1. Not paused initially
    assert_eq!(client.is_paused(), false);

    // 2. Pause
    set_time(&env, 100);
    let reason = String::from_str(&env, "Vulnerability found");
    client.pause(&admin, &reason);
    assert_eq!(client.is_paused(), true);

    // 3. Request unpause
    set_time(&env, 200);
    let unpause_after = client.request_unpause(&admin);
    assert_eq!(unpause_after, 200 + 86_400);

    // 4. Wait and execute unpause
    set_time(&env, unpause_after + 1);
    client.execute_unpause();
    assert_eq!(client.is_paused(), false);

    // 5. Can pause again
    set_time(&env, unpause_after + 100);
    let reason2 = String::from_str(&env, "Second incident");
    client.pause(&admin, &reason2);
    assert_eq!(client.is_paused(), true);

    // 6. History has all events
    let history = client.get_pause_history();
    assert_eq!(history.len(), 4); // pause, unpause_req, unpaused, pause
}

#[test]
fn test_multiple_guardians_lifecycle() {
    let (env, client, admin) = setup();
    let g2 = Address::generate(&env);
    let g3 = Address::generate(&env);

    client.add_guardian(&g2);
    client.add_guardian(&g3);

    // g2 pauses
    set_time(&env, 100);
    let reason = String::from_str(&env, "g2 detected issue");
    client.pause(&g2, &reason);
    assert_eq!(client.is_paused(), true);

    // g3 requests unpause
    set_time(&env, 200);
    client.request_unpause(&g3);

    // Anyone executes unpause after timelock
    set_time(&env, 200 + 86_400 + 1);
    client.execute_unpause();
    assert_eq!(client.is_paused(), false);

    // Admin removes g2
    client.remove_guardian(&g2);

    // g3 can still pause
    let reason2 = String::from_str(&env, "g3 pausing");
    client.pause(&g3, &reason2);
    assert_eq!(client.is_paused(), true);

    // g2 can no longer request unpause
    let result = client.try_request_unpause(&g2);
    assert!(result.is_err());
}
