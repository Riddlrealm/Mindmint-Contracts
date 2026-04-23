use soroban_sdk::{contracttype, Address, Env, Map, Symbol, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositeNFT {
    pub token_id: u32,
    pub owner: Address,
    pub parent_a: u32,
    pub parent_b: u32,
    pub inherited_traits: Map<Symbol, Symbol>,
    pub merge_timestamp: u64,
    pub is_burned: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitSelection {
    pub trait_key: Symbol,
    pub source: u32, // 1 for parent_a, 2 for parent_b
}

#[contracttype]
pub enum DataKey {
    Admin,
    RequiredTraits,
    Token(u32),
    TokenSupply,
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).expect("Admin not set")
}

pub fn set_required_traits(env: &Env, traits: &Vec<Symbol>) {
    env.storage().instance().set(&DataKey::RequiredTraits, traits);
}

pub fn get_required_traits(env: &Env) -> Vec<Symbol> {
    env.storage().instance().get(&DataKey::RequiredTraits).unwrap_or_else(|| Vec::new(env))
}

pub fn increment_supply(env: &Env) -> u32 {
    let mut current = env.storage().instance().get(&DataKey::TokenSupply).unwrap_or(0);
    current += 1;
    env.storage().instance().set(&DataKey::TokenSupply, &current);
    current
}

pub fn set_token(env: &Env, id: u32, token: &CompositeNFT) {
    env.storage().persistent().set(&DataKey::Token(id), token);
}

pub fn remove_token(env: &Env, id: u32) {
    env.storage().persistent().remove(&DataKey::Token(id));
}

pub fn get_token(env: &Env, id: u32) -> Option<CompositeNFT> {
    env.storage().persistent().get(&DataKey::Token(id))
}
