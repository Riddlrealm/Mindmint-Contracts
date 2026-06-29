use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Vec};

#[contracttype]
#[derive(Clone, Debug)]
pub struct DID {
    pub id: BytesN<32>,
    pub owner: Address,
    pub public_key: BytesN<32>,
    pub document: Bytes,
    pub created_at: u64,
    pub updated_at: u64,
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct VerifiableCredential {
    pub id: BytesN<32>,
    pub issuer: BytesN<32>,
    pub subject: Address,
    pub claim_type: Bytes,
    pub claims: Bytes,
    pub issued_at: u64,
    pub expiration: u64,
    pub revoked: bool,
    pub privacy_proof: Option<BytesN<32>>,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Delegation {
    pub delegator: Address,
    pub delegate: Address,
    pub permissions: Bytes,
    pub created_at: u64,
    pub active: bool,
}

#[contracttype]
pub enum DataKey {
    Initialized,
    Admin,
    UserDID(Address),
    DID(BytesN<32>),
    Credential(BytesN<32>),
    UserCredentials(Address),
    Attribute(Address, Bytes),
    Delegation(Address, Address), // (delegator, delegate)
}

pub(crate) fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub(crate) fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

pub(crate) fn has_did(env: &Env, user: &Address) -> bool {
    env.storage().persistent().has(&DataKey::UserDID(user.clone()))
}

pub(crate) fn set_user_did(env: &Env, user: &Address, did_id: &BytesN<32>) {
    env.storage().persistent().set(&DataKey::UserDID(user.clone()), did_id);
}

pub(crate) fn get_user_did(env: &Env, user: &Address) -> Option<BytesN<32>> {
    env.storage().persistent().get(&DataKey::UserDID(user.clone()))
}

pub(crate) fn set_did(env: &Env, did_id: &BytesN<32>, did: &DID) {
    env.storage().persistent().set(&DataKey::DID(did_id.clone()), did);
}

pub(crate) fn get_did(env: &Env, did_id: &BytesN<32>) -> Option<DID> {
    env.storage().persistent().get(&DataKey::DID(did_id.clone()))
}

pub(crate) fn set_credential(env: &Env, cred_id: &BytesN<32>, credential: &VerifiableCredential) {
    env.storage().persistent().set(&DataKey::Credential(cred_id.clone()), credential);
}

pub(crate) fn get_credential(env: &Env, cred_id: &BytesN<32>) -> Option<VerifiableCredential> {
    env.storage().persistent().get(&DataKey::Credential(cred_id.clone()))
}

pub(crate) fn add_user_credential(env: &Env, user: &Address, cred_id: &BytesN<32>) {
    let mut credentials = get_user_credentials(env, user);
    credentials.push_back(cred_id.clone());
    env.storage().persistent().set(&DataKey::UserCredentials(user.clone()), &credentials);
}

pub(crate) fn get_user_credentials(env: &Env, user: &Address) -> Vec<BytesN<32>> {
    env.storage().persistent().get(&DataKey::UserCredentials(user.clone())).unwrap_or_else(|| Vec::new(env))
}

pub(crate) fn has_attribute(env: &Env, user: &Address, key: &Bytes) -> bool {
    env.storage().persistent().has(&DataKey::Attribute(user.clone(), key.clone()))
}

pub(crate) fn set_attribute(env: &Env, user: &Address, key: &Bytes, value: &Bytes) {
    env.storage().persistent().set(&DataKey::Attribute(user.clone(), key.clone()), value);
}

pub(crate) fn get_attribute(env: &Env, user: &Address, key: &Bytes) -> Option<Bytes> {
    env.storage().persistent().get(&DataKey::Attribute(user.clone(), key.clone()))
}

pub(crate) fn set_delegation(env: &Env, delegator: &Address, delegate: &Address, delegation: &Delegation) {
    env.storage().persistent().set(&DataKey::Delegation(delegator.clone(), delegate.clone()), delegation);
}

pub(crate) fn get_delegation(env: &Env, delegator: &Address, delegate: &Address) -> Option<Delegation> {
    env.storage().persistent().get(&DataKey::Delegation(delegator.clone(), delegate.clone()))
}

pub(crate) fn is_delegate(env: &Env, delegate: &Address, delegator: &Address) -> bool {
    match get_delegation(env, delegator, delegate) {
        Some(d) => d.active,
        None => false,
    }
}