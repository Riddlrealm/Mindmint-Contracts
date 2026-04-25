use soroban_sdk::{Address, Env};
use crate::types::{CoCreation, DataKey};

/// Store a co-creation
pub fn set_co_creation(env: &Env, co_creation: &CoCreation) {
    env.storage().persistent().set(&DataKey::CoCreation(co_creation.id), co_creation);
}

/// Get a co-creation by ID
pub fn get_co_creation(env: &Env, id: u64) -> Option<CoCreation> {
    env.storage().persistent().get(&DataKey::CoCreation(id))
}

/// Get the next co-creation ID and increment counter
pub fn increment_co_creation_id(env: &Env) -> u64 {
    let key = DataKey::NextCoCreationId;
    let current: u64 = env.storage().instance().get(&key).unwrap_or(0);
    let next = current + 1;
    env.storage().instance().set(&key, &next);
    next
}

/// Check if an address has signed a co-creation
pub fn has_signed(env: &Env, co_creation_id: u64, signer: &Address) -> bool {
    env.storage().persistent().has(&DataKey::HasSigned(co_creation_id, signer.clone()))
}

/// Mark that an address has signed a co-creation
pub fn set_signed(env: &Env, co_creation_id: u64, signer: &Address) {
    env.storage().persistent().set(&DataKey::HasSigned(co_creation_id, signer.clone()), &true);
}

/// Remove signature marker
pub fn remove_signed(env: &Env, co_creation_id: u64, signer: &Address) {
    env.storage().persistent().remove(&DataKey::HasSigned(co_creation_id, signer.clone()));
}
