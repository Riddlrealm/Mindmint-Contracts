use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};
use soroban_sdk::token::Client as TokenClient;

fn create_token_contract(e: &Env, admin: &Address) -> Address {
    e.register_stellar_asset_contract(admin.clone())
}

#[test]
fn test_initialize_staking_pool() {
    let env = Env::default();
    let admin = Address::generate(&env);
    
    let staking_token = create_token_contract(&env, &admin);
    let reward_token = create_token_contract(&env, &admin);
    
    let contract = env.register(StakingRewardsContract, ());
    let client = StakingRewardsPoolClient::new(&env, &contract);
    
    // 20% APY, 30 day lockup, 10% early unstake penalty, auto-compound off by default
    client.initialize(
        &admin,
        &staking_token,
        &reward_token,
        &2000, // 20% APY
        &2592000, // 30 days in seconds
        &1000, // 10% penalty
        &false,
    );
    
    let config = client.get_config();
    assert_eq!(config.apy_bps, 2000);
    assert_eq!(config.lockup_period, 2592000);
    assert_eq!(config.early_unstake_penalty_bps, 1000);
}

#[test]
fn test_stake_tokens() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    let staking_token = create_token_contract(&env, &admin);
    let reward_token = create_token_contract(&env, &admin);
    
    let contract = env.register(StakingRewardsContract, ());
    let client = StakingRewardsPoolClient::new(&env, &contract);
    
    client.initialize(
        &admin,
        &staking_token,
        &reward_token,
        &2000,
        &2592000,
        &1000,
        &false,
    );
    
    // Mint tokens to user
    let token_client = TokenClient::new(&env, &staking_token);
    token_client.mint(&admin, &user, &1000);
    
    // Stake 100 tokens
    client.stake(&user, &100, &None);
    
    let staked = client.get_staked_amount(&user);
    assert_eq!(staked, 100);
    assert_eq!(client.get_total_staked(), 100);
    
    let history = client.get_staking_history(&user);
    assert_eq!(history.len(), 1);
}

#[test]
fn test_unstake_after_lockup() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    let staking_token = create_token_contract(&env, &admin);
    let reward_token = create_token_contract(&env, &admin);
    
    let contract = env.register(StakingRewardsContract, ());
    let client = StakingRewardsPoolClient::new(&env, &contract);
    
    let lockup_period = 2592000; // 30 days
    client.initialize(
        &admin,
        &staking_token,
        &reward_token,
        &2000,
        &lockup_period,
        &1000,
        &false,
    );
    
    // Mint and stake
    let token_client = TokenClient::new(&env, &staking_token);
    token_client.mint(&admin, &user, &1000);
    client.stake(&user, &100, &None);
    
    // Advance time past lockup
    env.ledger().set_timestamp(lockup_period + 1000);
    
    // Unstake
    let unstaked = client.unstake(&user, &100);
    
    assert_eq!(unstaked, 100); // No penalty
    assert_eq!(client.get_staked_amount(&user), 0);
    assert_eq!(client.get_total_staked(), 0);
}

#[test]
fn test_early_unstake_penalty() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    let staking_token = create_token_contract(&env, &admin);
    let reward_token = create_token_contract(&env, &admin);
    
    let contract = env.register(StakingRewardsContract, ());
    let client = StakingRewardsPoolClient::new(&env, &contract);
    
    let lockup_period = 2592000; // 30 days
    client.initialize(
        &admin,
        &staking_token,
        &reward_token,
        &2000,
        &lockup_period,
        &1000, // 10% penalty
        &false,
    );
    
    // Mint and stake
    let token_client = TokenClient::new(&env, &staking_token);
    token_client.mint(&admin, &user, &1000);
    client.stake(&user, &100, &None);
    
    // Try to unstake immediately (before lockup)
    let unstaked = client.unstake(&user, &100);
    
    assert_eq!(unstaked, 90); // 10% penalty applied
    assert_eq!(client.get_staked_amount(&user), 0);
}

#[test]
fn test_rewards_calculation() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    let staking_token = create_token_contract(&env, &admin);
    let reward_token = create_token_contract(&env, &admin);
    
    let contract = env.register(StakingRewardsContract, ());
    let client = StakingRewardsPoolClient::new(&env, &contract);
    
    client.initialize(
        &admin,
        &staking_token,
        &reward_token,
        &1000, // 10% APY
        &0, // No lockup
        &0,
        &false,
    );
    
    // Fund reward pool
    let reward_client = TokenClient::new(&env, &reward_token);
    reward_client.mint(&admin, &admin, &1000);
    client.fund_reward_pool(&1000);
    
    // Mint and stake
    let token_client = TokenClient::new(&env, &staking_token);
    token_client.mint(&admin, &user, &1000);
    client.stake(&user, &1000, &None);
    
    // Advance time by 1 year
    env.ledger().set_timestamp(31557600);
    
    // Calculate pending rewards
    let pending = client.get_pending_rewards(&user);
    // Should be ~10% of 1000 = 100 tokens
    assert!(pending > 95 && pending <= 100);
}

#[test]
fn test_auto_compound() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    let staking_token = create_token_contract(&env, &admin); // Same token for staking and rewards
    let reward_token = staking_token.clone();
    
    let contract = env.register(StakingRewardsContract, ());
    let client = StakingRewardsPoolClient::new(&env, &contract);
    
    client.initialize(
        &admin,
        &staking_token,
        &reward_token,
        &1000, // 10% APY
        &0,
        &0,
        &true, // Auto-compound by default
    );
    
    // Fund reward pool
    let token_client = TokenClient::new(&env, &staking_token);
    token_client.mint(&admin, &admin, &2000);
    client.fund_reward_pool(&1000);
    
    // Mint and stake with auto-compound
    token_client.mint(&admin, &user, &1000);
    client.stake(&user, &1000, &None);
    
    // Advance time and claim (which will auto-compound)
    env.ledger().set_timestamp(31557600);
    let claimed = client.claim_rewards(&user);
    
    // Staked amount should increase by the claimed rewards
    let position = client.get_position(&user);
    assert!(position.staked_amount > 1000);
}

#[test]
#[should_panic(expected = "code = 5")]
fn test_unstake_more_than_staked() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    let staking_token = create_token_contract(&env, &admin);
    let reward_token = create_token_contract(&env, &admin);
    
    let contract = env.register(StakingRewardsContract, ());
    let client = StakingRewardsPoolClient::new(&env, &contract);
    
    client.initialize(
        &admin,
        &staking_token,
        &reward_token,
        &2000,
        &2592000,
        &1000,
        &false,
    );
    
    // Mint and stake only 50 tokens
    let token_client = TokenClient::new(&env, &staking_token);
    token_client.mint(&admin, &user, &1000);
    client.stake(&user, &50, &None);
    
    // Try to unstake 100 tokens (more than staked)
    client.unstake(&user, &100);
}

#[test]
fn test_toggle_auto_compound() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    let staking_token = create_token_contract(&env, &admin);
    let reward_token = create_token_contract(&env, &admin);
    
    let contract = env.register(StakingRewardsContract, ());
    let client = StakingRewardsPoolClient::new(&env, &contract);
    
    client.initialize(
        &admin,
        &staking_token,
        &reward_token,
        &2000,
        &2592000,
        &1000,
        &false, // Default off
    );
    
    // Stake some tokens
    let token_client = TokenClient::new(&env, &staking_token);
    token_client.mint(&admin, &user, &1000);
    client.stake(&user, &100, &None);
    
    // Toggle auto-compound on
    client.toggle_auto_compound(&user, &true);
    
    let position = client.get_position(&user);
    assert!(position.auto_compound);
}