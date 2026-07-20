#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::Client as TokenClient,
    token::StellarAssetClient,
    Address, Env, String, Symbol,
};

fn create_token_contract<'a>(env: &Env, admin: &Address) -> (Address, TokenClient<'a>) {
    // register_stellar_asset_contract_v2 returns a helper object
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let address = sac.address(); // Extract the Address from the SAC object

    (address.clone(), TokenClient::new(env, &address))
}

#[test]
fn test_guild_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    // 1. Setup Identities
    let leader = Address::generate(&env);
    let officer = Address::generate(&env);
    let member = Address::generate(&env);
    let token_admin = Address::generate(&env);

    // 2. Setup Token (Treasury)
    let (token_addr, token_client) = create_token_contract(&env, &token_admin);
    let token_admin_client = StellarAssetClient::new(&env, &token_addr);

    // 3. Register and Initialize Guild
    let contract_id = env.register_contract(None, GuildContract);
    let client = GuildContractClient::new(&env, &contract_id);

    let guild_name = String::from_str(&env, "Stellar Knights");
    client.initialize(&leader, &guild_name, &token_addr);

    // 4. Test Membership & Roles
    client.join(&member);
    assert_eq!(client.get_role(&member), Some(Role::Member));

    client.set_role(&leader, &officer, &Role::Officer);
    assert_eq!(client.get_role(&officer), Some(Role::Officer));

    // 5. Test Treasury (Deposit)
    token_admin_client.mint(&member, &1000);
    client.deposit(&member, &1000);
    assert_eq!(token_client.balance(&contract_id), 1000);

    // 6. Test Resources
    let resource_name = Symbol::new(&env, "Gold");
    client.add_resource(&officer, &resource_name, &50);

    // 7. Test Voting
    env.ledger().set_timestamp(1000);
    let proposal_id = client.create_proposal(&officer, &2000);

    client.vote(&member, &proposal_id, &true);

    // 8. Test Disband
    token_admin_client.mint(&contract_id, &200); // Total 1200

    // Total members: 3 (Leader, Officer, Member)
    client.disband(&leader);

    // Each should receive 400
    assert_eq!(token_client.balance(&member), 400);
    assert_eq!(token_client.balance(&leader), 400);
    assert_eq!(token_client.balance(&officer), 400);
}

#[test]
#[should_panic(expected = "Officer or Leader only")]
fn test_unauthorized_resource_addition() {
    let env = Env::default();
    env.mock_all_auths();

    let leader = Address::generate(&env);
    let stranger = Address::generate(&env);
    let token_addr = Address::generate(&env);

    let contract_id = env.register_contract(None, GuildContract);
    let client = GuildContractClient::new(&env, &contract_id);

    client.initialize(&leader, &String::from_str(&env, "DAO"), &token_addr);

    client.add_resource(&stranger, &Symbol::new(&env, "Iron"), &100);
}

// ── Issue #19: role/membership gates on previously-unsecured entry points ──

#[test]
#[should_panic(expected = "Not a member")]
fn non_member_cannot_execute_withdrawal() {
    let env = Env::default();
    env.mock_all_auths();

    let leader = Address::generate(&env);
    let stranger = Address::generate(&env);
    let token_addr = Address::generate(&env);

    let contract_id = env.register_contract(None, GuildContract);
    let client = GuildContractClient::new(&env, &contract_id);
    client.initialize(&leader, &String::from_str(&env, "DAO"), &token_addr);

    // A non-member is rejected at the membership gate, before any proposal logic.
    client.execute_withdrawal(&stranger, &1);
}

#[test]
#[should_panic(expected = "Not a member")]
fn non_member_cannot_deposit() {
    let env = Env::default();
    env.mock_all_auths();

    let leader = Address::generate(&env);
    let stranger = Address::generate(&env);
    let token_addr = Address::generate(&env);

    let contract_id = env.register_contract(None, GuildContract);
    let client = GuildContractClient::new(&env, &contract_id);
    client.initialize(&leader, &String::from_str(&env, "DAO"), &token_addr);

    // Rejected at the membership gate, before the treasury transfer.
    client.deposit(&stranger, &1000);
}

#[test]
#[should_panic(expected = "Guild disbanded")]
fn disband_is_once_only() {
    let env = Env::default();
    env.mock_all_auths();

    let leader = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token_addr, _token_client) = create_token_contract(&env, &token_admin);
    let token_admin_client = StellarAssetClient::new(&env, &token_addr);

    let contract_id = env.register_contract(None, GuildContract);
    let client = GuildContractClient::new(&env, &contract_id);
    client.initialize(&leader, &String::from_str(&env, "DAO"), &token_addr);

    token_admin_client.mint(&contract_id, &100);

    client.disband(&leader); // first disband succeeds
    client.disband(&leader); // second must panic "Guild disbanded"
}

/// Regression guard for the design decision in the audit: execution gates on
/// membership, not officer — a plain Member can execute an approved withdrawal.
#[test]
fn member_can_execute_approved_withdrawal() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let leader = Address::generate(&env);
    let member_a = Address::generate(&env);
    let member_b = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token_addr, token_client) = create_token_contract(&env, &token_admin);
    let token_admin_client = StellarAssetClient::new(&env, &token_addr);

    let contract_id = env.register_contract(None, GuildContract);
    let client = GuildContractClient::new(&env, &contract_id);
    client.initialize(&leader, &String::from_str(&env, "DAO"), &token_addr);

    client.join(&member_a);
    client.join(&member_b);

    // Fund the treasury above the withdrawal threshold (10_000).
    token_admin_client.mint(&contract_id, &20_000);

    // Leader proposes a large withdrawal -> creates a proposal (returns Some(id)).
    let wid = client
        .withdraw(&leader, &20_000i128, &Some(100_000u64))
        .expect("large withdrawal should create a proposal");

    // Two of three members approve -> yes (2) > total_members (3) / 2.
    client.vote_withdrawal(&member_a, &wid, &true);
    client.vote_withdrawal(&member_b, &wid, &true);

    // A plain Member (not officer/leader) can execute the approved withdrawal.
    client.execute_withdrawal(&member_a, &wid);

    // Funds went to the proposal's officer (the leader who proposed it).
    assert_eq!(token_client.balance(&leader), 20_000);
    assert_eq!(token_client.balance(&contract_id), 0);
}
