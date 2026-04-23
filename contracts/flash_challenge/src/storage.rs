use soroban_sdk::{contracttype, Address, Env, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChallengeStatus {
    Scheduled,
    Active,
    Completed,
    Expired,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlashChallenge {
    pub id: u32,
    pub puzzle_id: u32,
    pub reward_pool: i128,
    pub max_winners: u32,
    pub start_at: u64,
    pub end_at: u64,
    pub winners: Vec<Address>,
    pub status: ChallengeStatus,
}

#[contracttype]
pub enum DataKey {
    Admin,
    TokenAddress,
    OracleAddress,
    Challenge(u32),
    ChallengeCount,
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).expect("Admin not initialized")
}

pub fn set_token_address(env: &Env, token: &Address) {
    env.storage().instance().set(&DataKey::TokenAddress, token);
}

pub fn get_token_address(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::TokenAddress).expect("Token not initialized")
}

pub fn set_oracle_address(env: &Env, oracle: &Address) {
    env.storage().instance().set(&DataKey::OracleAddress, oracle);
}

pub fn get_oracle_address(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::OracleAddress).expect("Oracle not initialized")
}

pub fn set_challenge(env: &Env, id: u32, challenge: &FlashChallenge) {
    env.storage().persistent().set(&DataKey::Challenge(id), challenge);
}

pub fn get_challenge(env: &Env, id: u32) -> Option<FlashChallenge> {
    env.storage().persistent().get(&DataKey::Challenge(id))
}

pub fn increment_challenge_count(env: &Env) -> u32 {
    let mut count = env.storage().instance().get(&DataKey::ChallengeCount).unwrap_or(0);
    count += 1;
    env.storage().instance().set(&DataKey::ChallengeCount, &count);
    count
}
