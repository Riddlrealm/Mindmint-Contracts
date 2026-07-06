use soroban_sdk::{contracttype, Address, BytesN, Env, Vec};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Paused,
    UpgradeHistory,
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .expect("Admin not set")
}

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&DataKey::Paused, &paused);
}

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

pub fn push_upgrade_history(env: &Env, hash: &BytesN<32>) {
    let mut history: Vec<BytesN<32>> = env
        .storage()
        .instance()
        .get(&DataKey::UpgradeHistory)
        .unwrap_or(Vec::new(env));
    history.push_back(hash.clone());
    env.storage()
        .instance()
        .set(&DataKey::UpgradeHistory, &history);
}

pub fn get_upgrade_history(env: &Env) -> Vec<BytesN<32>> {
    env.storage()
        .instance()
        .get(&DataKey::UpgradeHistory)
        .unwrap_or(Vec::new(env))
}
