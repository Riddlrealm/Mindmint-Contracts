#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger}, 
    Address, Env, String, Symbol,
    token::StellarAssetClient,
    token::Client as TokenClient,
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