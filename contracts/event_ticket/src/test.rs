#![cfg(test)]
extern crate std;
use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger, Address, Symbol};

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EventTicketContract);
    let client = EventTicketContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    client.initialize(&admin, &oracle);

    // Test duplicate initialization
    let result = client.try_initialize(&admin, &oracle);
    assert!(result.is_err());
}

#[test]
fn test_create_event() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EventTicketContract);
    let client = EventTicketContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    client.initialize(&admin, &oracle);

    let name = Symbol::new(&env, "Tournament");
    let start_at = 1000;
    let end_at = 5000;
    let max_tickets = 100;

    let event_id = client.create_event(&name, &start_at, &end_at, &max_tickets);
    assert_eq!(event_id, 1);

    // Verify event was created by checking it exists via attendance
    let (_attended, issued) = client.get_attendance(&event_id);
    assert_eq!(issued, 0);
}

#[test]
fn test_issue_ticket() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EventTicketContract);
    let client = EventTicketContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    client.initialize(&admin, &oracle);

    let name = Symbol::new(&env, "Tournament");
    let start_at = 1000;
    let end_at = 5000;
    let max_tickets = 100;

    let event_id = client.create_event(&name, &start_at, &end_at, &max_tickets);

    let recipient = Address::generate(&env);
    let tier = TicketTier::General;

    let token_id = client.issue_ticket(&event_id, &recipient, &tier);
    assert_eq!(token_id, 1);

    // Verify ticket was issued by checking attendance
    let (_attended, issued) = client.get_attendance(&event_id);
    assert_eq!(issued, 1);

    // Verify ticket is in holder's list
    let tickets = client.get_tickets(&recipient);
    assert_eq!(tickets.len(), 1);
    assert_eq!(tickets.get(0), Some(token_id));
}

#[test]
fn test_max_tickets_cap() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EventTicketContract);
    let client = EventTicketContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    client.initialize(&admin, &oracle);

    let name = Symbol::new(&env, "Tournament");
    let start_at = 1000;
    let end_at = 5000;
    let max_tickets = 2;

    let event_id = client.create_event(&name, &start_at, &end_at, &max_tickets);

    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);
    let recipient3 = Address::generate(&env);
    let tier = TicketTier::General;

    client.issue_ticket(&event_id, &recipient1, &tier);
    client.issue_ticket(&event_id, &recipient2, &tier);

    // Try to issue beyond max
    let result = client.try_issue_ticket(&event_id, &recipient3, &tier);
    assert!(result.is_err());
}

#[test]
fn test_transfer_before_start() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EventTicketContract);
    let client = EventTicketContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    client.initialize(&admin, &oracle);

    let name = Symbol::new(&env, "Tournament");
    let start_at = 5000;
    let end_at = 10000;
    let max_tickets = 100;

    let event_id = client.create_event(&name, &start_at, &end_at, &max_tickets);

    let holder1 = Address::generate(&env);
    let holder2 = Address::generate(&env);
    let tier = TicketTier::General;

    let token_id = client.issue_ticket(&event_id, &holder1, &tier);

    // Set time before event start
    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    client.transfer_ticket(&token_id, &holder2);

    // Verify ticket was transferred by checking holder lists
    let tickets1 = client.get_tickets(&holder1);
    assert_eq!(tickets1.len(), 0);

    let tickets2 = client.get_tickets(&holder2);
    assert_eq!(tickets2.len(), 1);
    assert_eq!(tickets2.get(0), Some(token_id));
}

#[test]
fn test_transfer_after_start_rejection() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EventTicketContract);
    let client = EventTicketContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    client.initialize(&admin, &oracle);

    let name = Symbol::new(&env, "Tournament");
    let start_at = 1000;
    let end_at = 5000;
    let max_tickets = 100;

    let event_id = client.create_event(&name, &start_at, &end_at, &max_tickets);

    let holder1 = Address::generate(&env);
    let holder2 = Address::generate(&env);
    let tier = TicketTier::General;

    let token_id = client.issue_ticket(&event_id, &holder1, &tier);

    // Set time after event start
    env.ledger().with_mut(|li| {
        li.timestamp = 2000;
    });

    let result = client.try_transfer_ticket(&token_id, &holder2);
    assert!(result.is_err());
}

#[test]
fn test_check_in() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EventTicketContract);
    let client = EventTicketContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    client.initialize(&admin, &oracle);

    let name = Symbol::new(&env, "Tournament");
    let start_at = 1000;
    let end_at = 5000;
    let max_tickets = 100;

    let event_id = client.create_event(&name, &start_at, &end_at, &max_tickets);

    let holder = Address::generate(&env);
    let tier = TicketTier::General;

    let token_id = client.issue_ticket(&event_id, &holder, &tier);

    // Set time during event
    env.ledger().with_mut(|li| {
        li.timestamp = 2000;
    });

    client.check_in(&token_id);

    // Verify check-in by checking attendance
    let (attended, issued) = client.get_attendance(&event_id);
    assert_eq!(attended, 1);
    assert_eq!(issued, 1);
}

#[test]
fn test_check_in_before_event() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EventTicketContract);
    let client = EventTicketContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    client.initialize(&admin, &oracle);

    let name = Symbol::new(&env, "Tournament");
    let start_at = 1000;
    let end_at = 5000;
    let max_tickets = 100;

    let event_id = client.create_event(&name, &start_at, &end_at, &max_tickets);

    let holder = Address::generate(&env);
    let tier = TicketTier::General;

    let token_id = client.issue_ticket(&event_id, &holder, &tier);

    // Set time before event
    env.ledger().with_mut(|li| {
        li.timestamp = 500;
    });

    let result = client.try_check_in(&token_id);
    assert!(result.is_err());
}

#[test]
fn test_check_in_after_event() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EventTicketContract);
    let client = EventTicketContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    client.initialize(&admin, &oracle);

    let name = Symbol::new(&env, "Tournament");
    let start_at = 1000;
    let end_at = 5000;
    let max_tickets = 100;

    let event_id = client.create_event(&name, &start_at, &end_at, &max_tickets);

    let holder = Address::generate(&env);
    let tier = TicketTier::General;

    let token_id = client.issue_ticket(&event_id, &holder, &tier);

    // Set time after event
    env.ledger().with_mut(|li| {
        li.timestamp = 6000;
    });

    let result = client.try_check_in(&token_id);
    assert!(result.is_err());
}

#[test]
fn test_double_check_in() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EventTicketContract);
    let client = EventTicketContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    client.initialize(&admin, &oracle);

    let name = Symbol::new(&env, "Tournament");
    let start_at = 1000;
    let end_at = 5000;
    let max_tickets = 100;

    let event_id = client.create_event(&name, &start_at, &end_at, &max_tickets);

    let holder = Address::generate(&env);
    let tier = TicketTier::General;

    let token_id = client.issue_ticket(&event_id, &holder, &tier);

    // Set time during event
    env.ledger().with_mut(|li| {
        li.timestamp = 2000;
    });

    client.check_in(&token_id);

    // Try to check in again
    let result = client.try_check_in(&token_id);
    assert!(result.is_err());
}

#[test]
fn test_get_tickets() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EventTicketContract);
    let client = EventTicketContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    client.initialize(&admin, &oracle);

    let name = Symbol::new(&env, "Tournament");
    let start_at = 1000;
    let end_at = 5000;
    let max_tickets = 100;

    let event_id = client.create_event(&name, &start_at, &end_at, &max_tickets);

    let holder = Address::generate(&env);
    let tier = TicketTier::General;

    let token_id1 = client.issue_ticket(&event_id, &holder, &tier);
    let token_id2 = client.issue_ticket(&event_id, &holder, &tier);

    let tickets = client.get_tickets(&holder);
    assert_eq!(tickets.len(), 2);
    assert_eq!(tickets.get(0).unwrap(), token_id1);
    assert_eq!(tickets.get(1).unwrap(), token_id2);
}

#[test]
fn test_attendance_stats() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EventTicketContract);
    let client = EventTicketContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    client.initialize(&admin, &oracle);

    let name = Symbol::new(&env, "Tournament");
    let start_at = 1000;
    let end_at = 5000;
    let max_tickets = 100;

    let event_id = client.create_event(&name, &start_at, &end_at, &max_tickets);

    let holder1 = Address::generate(&env);
    let holder2 = Address::generate(&env);
    let holder3 = Address::generate(&env);
    let tier = TicketTier::General;

    let _token_id1 = client.issue_ticket(&event_id, &holder1, &tier);
    let _token_id2 = client.issue_ticket(&event_id, &holder2, &tier);
    let _token_id3 = client.issue_ticket(&event_id, &holder3, &tier);

    // Verify all tickets were issued
    let (attended, issued) = client.get_attendance(&event_id);
    assert_eq!(attended, 0);
    assert_eq!(issued, 3);
}

#[test]
fn test_vip_and_backstage_tiers() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EventTicketContract);
    let client = EventTicketContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);

    client.initialize(&admin, &oracle);

    let name = Symbol::new(&env, "Tournament");
    let start_at = 1000;
    let end_at = 5000;
    let max_tickets = 100;

    let event_id = client.create_event(&name, &start_at, &end_at, &max_tickets);

    let holder1 = Address::generate(&env);
    let holder2 = Address::generate(&env);

    let token_id1 = client.issue_ticket(&event_id, &holder1, &TicketTier::VIP);
    let token_id2 = client.issue_ticket(&event_id, &holder2, &TicketTier::Backstage);

    // Verify both tickets were issued
    let (_attended, issued) = client.get_attendance(&event_id);
    assert_eq!(issued, 2);

    // Verify tickets are in respective holders' lists
    let tickets1 = client.get_tickets(&holder1);
    assert_eq!(tickets1.len(), 1);
    assert_eq!(tickets1.get(0), Some(token_id1));

    let tickets2 = client.get_tickets(&holder2);
    assert_eq!(tickets2.len(), 1);
    assert_eq!(tickets2.get(0), Some(token_id2));
}
