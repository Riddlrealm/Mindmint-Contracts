#![cfg(test)]
use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String, Vec,
};

#[test]
fn test_escrow_creation() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let party1 = Address::generate(&env);
    let party2 = Address::generate(&env);
    let token = Address::generate(&env);

    let parties = Vec::from_array(&env, [party1.clone(), party2.clone()]);
    let amounts = Vec::from_array(&env, [1000i128, 2000i128]);
    let conditions = Vec::from_array(&env, [ReleaseCondition::AllPartiesApprove]);

    let escrow_id = client.create_escrow(
        &creator,
        &parties,
        &token,
        &amounts,
        &conditions,
        &None,
        &3600u64,
    );

    assert_eq!(escrow_id, 1);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.state, EscrowState::Created);
    assert_eq!(escrow.parties, parties);
    assert_eq!(escrow.amounts, amounts);
}

#[test]
fn test_escrow_state_transitions() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let party1 = Address::generate(&env);
    let party2 = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let token = Address::generate(&env);

    let parties = Vec::from_array(&env, [party1.clone(), party2.clone()]);
    let amounts = Vec::from_array(&env, [1000i128, 2000i128]);
    let conditions = Vec::from_array(&env, [ReleaseCondition::AllPartiesApprove]);

    let escrow_id = client.create_escrow(
        &creator,
        &parties,
        &token,
        &amounts,
        &conditions,
        &Some(arbitrator.clone()),
        &3600u64,
    );

    // Manually set escrow to active state using contract context
    env.as_contract(&contract_id, || {
        let mut escrow: EscrowData = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .unwrap();
        escrow.state = EscrowState::Active;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);
    });

    client.dispute(&party1, &escrow_id, &String::from_str(&env, "Test dispute"));

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.state, EscrowState::Disputed);

    client.resolve_dispute(&arbitrator, &escrow_id, &DisputeResolution::Refund);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.state, EscrowState::Refunded);
}

#[test]
fn test_timeout_functionality() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let party1 = Address::generate(&env);
    let party2 = Address::generate(&env);
    let token = Address::generate(&env);

    let parties = Vec::from_array(&env, [party1.clone(), party2.clone()]);
    let amounts = Vec::from_array(&env, [1000i128, 2000i128]);
    let conditions = Vec::from_array(&env, [ReleaseCondition::AllPartiesApprove]);

    let escrow_id = client.create_escrow(
        &creator,
        &parties,
        &token,
        &amounts,
        &conditions,
        &None,
        &100u64,
    );

    // Set escrow to active state using contract context
    env.as_contract(&contract_id, || {
        let mut escrow: EscrowData = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .unwrap();
        escrow.state = EscrowState::Active;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);
    });

    // Advance time past timeout
    env.ledger().with_mut(|li| li.timestamp = 200);

    client.refund_timeout(&escrow_id);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.state, EscrowState::Refunded);
}

#[test]
fn test_approval_logic() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let party1 = Address::generate(&env);
    let party2 = Address::generate(&env);
    let party3 = Address::generate(&env);
    let token = Address::generate(&env);

    let parties = Vec::from_array(&env, [party1.clone(), party2.clone(), party3.clone()]);
    let amounts = Vec::from_array(&env, [1000i128, 2000i128, 1500i128]);
    let conditions = Vec::from_array(&env, [ReleaseCondition::MajorityApprove]);

    let escrow_id = client.create_escrow(
        &creator,
        &parties,
        &token,
        &amounts,
        &conditions,
        &None,
        &3600u64,
    );

    // Set escrow to active state using contract context
    env.as_contract(&contract_id, || {
        let mut escrow: EscrowData = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .unwrap();
        escrow.state = EscrowState::Active;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);
    });

    // Test majority approval
    client.approve(&party1, &escrow_id);
    client.approve(&party2, &escrow_id);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.state, EscrowState::Released);
}

#[test]
fn test_error_conditions() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, EscrowContract);
    let client = EscrowContractClient::new(&env, &contract_id);

    // Test escrow not found
    let result = client.get_escrow(&999);
    assert_eq!(result, None);

    // Test invalid parties
    let creator = Address::generate(&env);
    let token = Address::generate(&env);
    let empty_parties = vec![&env];
    let empty_amounts = vec![&env];
    let conditions = Vec::from_array(&env, [ReleaseCondition::AllPartiesApprove]);

    let result = client.try_create_escrow(
        &creator,
        &empty_parties,
        &token,
        &empty_amounts,
        &conditions,
        &None,
        &3600u64,
    );

    assert_eq!(result, Err(Ok(EscrowError::InvalidParties)));
}
