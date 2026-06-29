use soroban_sdk::{contracttype, Address, Env, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StakingAction {
    Stake = 0,
    Unstake = 1,
    Claim = 2,
    Compound = 3,
    Penalty = 4,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct StakingHistoryEntry {
    pub amount: i128,
    pub timestamp: u64,
    pub action: StakingAction,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct StakingPosition {
    pub user: Address,
    pub staked_amount: i128,
    pub staked_at: u64,
    pub last_update_time: u64,
    pub pending_rewards: i128,
    pub total_claimed: i128,
    pub auto_compound: bool,
}

#[contracttype]
pub enum DataKey {
    Initialized,
    Admin,
    StakingToken,
    RewardToken,
    ApyBps,
    LockupPeriod,
    EarlyUnstakePenaltyBps,
    AutoCompoundDefault,
    TotalStaked,
    Position(Address),
    History(Address),
}

impl Default for StakingPosition {
    fn default() -> Self {
        StakingPosition {
            user: Address::from_array(&[0; 32]),
            staked_amount: 0,
            staked_at: 0,
            last_update_time: 0,
            pending_rewards: 0,
            total_claimed: 0,
            auto_compound: false,
        }
    }
}

pub(crate) fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub(crate) fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

pub(crate) fn set_staking_token(env: &Env, token: &Address) {
    env.storage().instance().set(&DataKey::StakingToken, token);
}

pub(crate) fn get_staking_token(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::StakingToken).unwrap()
}

pub(crate) fn set_reward_token(env: &Env, token: &Address) {
    env.storage().instance().set(&DataKey::RewardToken, token);
}

pub(crate) fn get_reward_token(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::RewardToken).unwrap()
}

pub(crate) fn set_apy_bps(env: &Env, bps: &u32) {
    env.storage().instance().set(&DataKey::ApyBps, bps);
}

pub(crate) fn get_apy_bps(env: &Env) -> u32 {
    env.storage().instance().get(&DataKey::ApyBps).unwrap()
}

pub(crate) fn set_lockup_period(env: &Env, period: &u64) {
    env.storage().instance().set(&DataKey::LockupPeriod, period);
}

pub(crate) fn get_lockup_period(env: &Env) -> u64 {
    env.storage().instance().get(&DataKey::LockupPeriod).unwrap()
}

pub(crate) fn set_early_unstake_penalty_bps(env: &Env, bps: &u32) {
    env.storage().instance().set(&DataKey::EarlyUnstakePenaltyBps, bps);
}

pub(crate) fn get_early_unstake_penalty_bps(env: &Env) -> u32 {
    env.storage().instance().get(&DataKey::EarlyUnstakePenaltyBps).unwrap()
}

pub(crate) fn set_auto_compound_default(env: &Env, default: &bool) {
    env.storage().instance().set(&DataKey::AutoCompoundDefault, default);
}

pub(crate) fn get_auto_compound_default(env: &Env) -> bool {
    env.storage().instance().get(&DataKey::AutoCompoundDefault).unwrap()
}

pub(crate) fn set_total_staked(env: &Env, amount: &i128) {
    env.storage().instance().set(&DataKey::TotalStaked, amount);
}

pub(crate) fn get_total_staked(env: &Env) -> i128 {
    env.storage().instance().get(&DataKey::TotalStaked).unwrap_or(0)
}

pub(crate) fn set_position(env: &Env, user: &Address, position: &StakingPosition) {
    env.storage().persistent().set(&DataKey::Position(user.clone()), position);
}

pub(crate) fn get_position(env: &Env, user: &Address) -> StakingPosition {
    env.storage().persistent().get(&DataKey::Position(user.clone())).unwrap_or_else(|| StakingPosition {
        user: user.clone(),
        staked_amount: 0,
        staked_at: 0,
        last_update_time: 0,
        pending_rewards: 0,
        total_claimed: 0,
        auto_compound: get_auto_compound_default(env),
    })
}

pub(crate) fn add_staking_history(env: &Env, user: &Address, amount: i128, timestamp: u64, action: StakingAction) {
    let mut history = get_staking_history(env, user);
    history.push_back(StakingHistoryEntry {
        amount,
        timestamp,
        action,
    });
    env.storage().persistent().set(&DataKey::History(user.clone()), &history);
}

pub(crate) fn get_staking_history(env: &Env, user: &Address) -> Vec<StakingHistoryEntry> {
    env.storage().persistent().get(&DataKey::History(user.clone())).unwrap_or_else(|| Vec::new(env))
}

/// Calculate rewards for a user based on time staked and APY
pub(crate) fn calculate_rewards(env: &Env, user: &Address) -> i128 {
    let position = get_position(env, user);
    if position.staked_amount == 0 || position.last_update_time == 0 {
        return 0;
    }

    let current_time = env.ledger().timestamp();
    let time_elapsed = current_time - position.last_update_time;
    
    if time_elapsed == 0 {
        return 0;
    }

    // Calculate annual rewards: staked_amount * (apy_bps / 10000)
    let apy_bps = get_apy_bps(env);
    let annual_rewards = position.staked_amount * apy_bps as i128 / 10000;
    
    // Convert to proportional rewards for the time elapsed
    // 365.25 days in a year = 31557600 seconds
    const YEAR_SECONDS: u64 = 31557600;
    let rewards = annual_rewards * time_elapsed as i128 / YEAR_SECONDS as i128;

    rewards
}

/// Calculate and accrue rewards for all stakers (call before changing APY)
pub(crate) fn snap_all_rewards(env: &Env) {
    // In a real implementation, this would iterate through all stakers
    // For this implementation, rewards are calculated on-demand during user interactions
    // which is more gas-efficient for Soroban
}