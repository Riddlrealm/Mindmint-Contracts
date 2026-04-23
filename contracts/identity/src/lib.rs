#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, String, Symbol,
};

const USERNAME_MIN: u32 = 3;
const USERNAME_MAX: u32 = 20;
const TRANSFER_COOLDOWN_SECONDS: u64 = 30 * 24 * 60 * 60; // 30 days

#[contracttype]
#[derive(Clone, Debug)]
pub enum DataKey {
    Config,
    Username(String),
    Address(Address),
    TransferCooldown(Address),
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Config {
    pub admin: Address,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SocialLinks {
    pub twitter: Option<String>,
    pub discord: Option<String>,
    pub github: Option<String>,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerIdentity {
    pub address: Address,
    pub username: String,
    pub avatar_hash: Option<String>,
    pub bio_hash: Option<String>,
    pub social_links: SocialLinks,
    pub registered_at: u64,
    pub verified: bool,
}

#[contract]
pub struct IdentityContract;

#[contractimpl]
impl IdentityContract {
    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();
        if env.storage().persistent().has(&DataKey::Config) {
            panic!("Already initialized");
        }
        env.storage().persistent().set(&DataKey::Config, &Config { admin });
    }

    pub fn register(env: Env, caller: Address, username: String) {
        caller.require_auth();
        Self::validate_username(&username);

        if env.storage().persistent().has(&DataKey::Username(username.clone())) {
            panic!("Username already taken");
        }
        if env.storage().persistent().has(&DataKey::Address(caller.clone())) {
            panic!("Address already has identity");
        }

        let now = env.ledger().timestamp();
        let identity = PlayerIdentity {
            address: caller.clone(),
            username: username.clone(),
            avatar_hash: None,
            bio_hash: None,
            social_links: SocialLinks {
                twitter: None,
                discord: None,
                github: None,
            },
            registered_at: now,
            verified: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Username(username.clone()), &caller);
        env.storage()
            .persistent()
            .set(&DataKey::Address(caller.clone()), &identity);

        env.events().publish(
            (Symbol::new(&env, "IdentityRegistered"), caller.clone()),
            username,
        );
    }

    pub fn update_profile(
        env: Env,
        caller: Address,
        avatar_hash: Option<String>,
        bio_hash: Option<String>,
        social_links: SocialLinks,
    ) {
        caller.require_auth();

        let mut identity = Self::must_get_identity_by_address(&env, &caller);
        identity.avatar_hash = avatar_hash;
        identity.bio_hash = bio_hash;
        identity.social_links = social_links;

        env.storage()
            .persistent()
            .set(&DataKey::Address(caller.clone()), &identity);

        env.events().publish(
            (Symbol::new(&env, "ProfileUpdated"), caller.clone()),
            identity.username,
        );
    }

    pub fn verify_identity(env: Env, admin: Address, target: Address) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut identity = Self::must_get_identity_by_address(&env, &target);
        identity.verified = true;

        env.storage()
            .persistent()
            .set(&DataKey::Address(target), &identity);
    }

    pub fn resolve_username(env: Env, username: String) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Username(username))
    }

    pub fn resolve_address(env: Env, address: Address) -> Option<PlayerIdentity> {
        env.storage().persistent().get(&DataKey::Address(address))
    }

    pub fn transfer_username(env: Env, caller: Address, new_owner: Address) {
        caller.require_auth();

        let mut identity = Self::must_get_identity_by_address(&env, &caller);
        let username = identity.username.clone();

        // Enforce cooldown
        if let Some(last_transfer) = env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&DataKey::TransferCooldown(caller.clone()))
        {
            let now = env.ledger().timestamp();
            if now < last_transfer + TRANSFER_COOLDOWN_SECONDS {
                panic!("Transfer cooldown not elapsed");
            }
        }

        // Remove old mappings
        env.storage()
            .persistent()
            .remove(&DataKey::Username(username.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::Address(caller.clone()));

        // Update identity address
        identity.address = new_owner.clone();

        // Set new mappings
        env.storage()
            .persistent()
            .set(&DataKey::Username(username.clone()), &new_owner);
        env.storage()
            .persistent()
            .set(&DataKey::Address(new_owner.clone()), &identity);

        // Record cooldown timestamp for the new owner to prevent immediate re-transfer
        env.storage()
            .persistent()
            .set(&DataKey::TransferCooldown(new_owner.clone()), &env.ledger().timestamp());

        env.events().publish(
            (Symbol::new(&env, "UsernameTransferred"), caller.clone()),
            (new_owner, username),
        );
    }

    fn validate_username(username: &String) {
        let len = username.len() as u32;
        if len < USERNAME_MIN || len > USERNAME_MAX {
            panic!("Username length must be 3-20 characters");
        }
        // Simple length check suffices; alphanumeric enforcement can be done at UI layer
    }

    fn must_get_identity_by_address(env: &Env, address: &Address) -> PlayerIdentity {
        env.storage()
            .persistent()
            .get(&DataKey::Address(address.clone()))
            .unwrap_or_else(|| panic!("Identity not found"))
    }

    fn assert_admin(env: &Env, user: &Address) {
        let cfg = Self::get_config(env);
        if cfg.admin != *user {
            panic!("Admin only");
        }
    }

    fn get_config(env: &Env) -> Config {
        env.storage()
            .persistent()
            .get(&DataKey::Config)
            .unwrap_or_else(|| panic!("Not initialized"))
    }
}

#[cfg(test)]
mod test;
