use soroban_sdk::{contracttype, Address, Env, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Delegation {
    pub delegator: Address,
    pub delegatee: Address,
    pub delegated_at: u64,
    pub active: bool,
}

#[contracttype]
pub enum DataKey {
    Delegation(Address),
    Delegators(Address),
    TokenAddress,
}

pub fn set_token_address(env: &Env, token: &Address) {
    env.storage().instance().set(&DataKey::TokenAddress, token);
}

pub fn get_token_address(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::TokenAddress).expect("Token not initialized")
}

pub fn set_delegation(env: &Env, delegator: &Address, delegation: &Delegation) {
    env.storage().persistent().set(&DataKey::Delegation(delegator.clone()), delegation);
}

pub fn get_delegation(env: &Env, delegator: &Address) -> Option<Delegation> {
    env.storage().persistent().get(&DataKey::Delegation(delegator.clone()))
}

pub fn add_delegator(env: &Env, delegatee: &Address, delegator: &Address) {
    let mut delegators = get_delegators(env, delegatee);
    if !delegators.contains(delegator) {
        delegators.push_back(delegator.clone());
        env.storage().persistent().set(&DataKey::Delegators(delegatee.clone()), &delegators);
    }
}

pub fn remove_delegator(env: &Env, delegatee: &Address, delegator: &Address) {
    let mut delegators = get_delegators(env, delegatee);
    if let Some(idx) = delegators.first_index_of(delegator) {
        delegators.remove(idx);
        env.storage().persistent().set(&DataKey::Delegators(delegatee.clone()), &delegators);
    }
}

pub fn get_delegators(env: &Env, delegatee: &Address) -> Vec<Address> {
    env.storage().persistent().get(&DataKey::Delegators(delegatee.clone())).unwrap_or_else(|| Vec::new(env))
}
