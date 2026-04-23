use soroban_sdk::{contracttype, Address, Env};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MiningPosition {
    pub provider: Address,
    pub lp_tokens: i128,
    pub staked_at: u64,
    pub last_claim_epoch: u32,
    pub total_claimed: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MiningEpoch {
    pub epoch_id: u32,
    pub start_at: u64,
    pub end_at: u64,
    pub reward_budget: i128,
    pub total_lp_staked: i128,
    pub distributed: bool,
}

#[contracttype]
pub enum DataKey {
    Admin,
    LpToken,
    RewardToken,
    CurrentEpochId,
    GlobalLpStaked,
    Epoch(u32),
    Position(Address),
    Claimed(Address, u32), // tracks if user claimed an epoch
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).expect("Admin not initialized")
}

pub fn set_lp_token(env: &Env, token: &Address) {
    env.storage().instance().set(&DataKey::LpToken, token);
}

pub fn get_lp_token(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::LpToken).expect("LP Token not set")
}

pub fn set_reward_token(env: &Env, token: &Address) {
    env.storage().instance().set(&DataKey::RewardToken, token);
}

pub fn get_reward_token(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::RewardToken).expect("Reward Token not set")
}

pub fn set_current_epoch_id(env: &Env, id: u32) {
    env.storage().instance().set(&DataKey::CurrentEpochId, &id);
}

pub fn get_current_epoch_id(env: &Env) -> u32 {
    env.storage().instance().get(&DataKey::CurrentEpochId).unwrap_or(0)
}

pub fn set_global_lp_staked(env: &Env, amount: i128) {
    env.storage().instance().set(&DataKey::GlobalLpStaked, &amount);
}

pub fn get_global_lp_staked(env: &Env) -> i128 {
    env.storage().instance().get(&DataKey::GlobalLpStaked).unwrap_or(0)
}

pub fn set_epoch(env: &Env, id: u32, epoch: &MiningEpoch) {
    env.storage().persistent().set(&DataKey::Epoch(id), epoch);
}

pub fn get_epoch(env: &Env, id: u32) -> Option<MiningEpoch> {
    env.storage().persistent().get(&DataKey::Epoch(id))
}

pub fn set_position(env: &Env, provider: &Address, pos: &MiningPosition) {
    env.storage().persistent().set(&DataKey::Position(provider.clone()), pos);
}

pub fn get_position(env: &Env, provider: &Address) -> MiningPosition {
    env.storage().persistent().get(&DataKey::Position(provider.clone())).unwrap_or_else(|| MiningPosition {
        provider: provider.clone(),
        lp_tokens: 0,
        staked_at: 0,
        last_claim_epoch: 0,
        total_claimed: 0,
    })
}

pub fn set_has_claimed(env: &Env, provider: &Address, epoch_id: u32) {
    env.storage().persistent().set(&DataKey::Claimed(provider.clone(), epoch_id), &true);
}

pub fn has_claimed(env: &Env, provider: &Address, epoch_id: u32) -> bool {
    env.storage().persistent().get(&DataKey::Claimed(provider.clone(), epoch_id)).unwrap_or(false)
}
