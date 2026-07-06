#![no_std]

mod storage;

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Vec};
use storage::{
    get_admin, get_upgrade_history, is_paused, push_upgrade_history, set_admin, set_paused, DataKey,
};

#[contract]
pub struct ProxyContract;

#[contractimpl]
impl ProxyContract {
    /// Initialize the proxy with an admin address.
    pub fn initialize(env: Env, admin: Address) {
        // Only allow first initialization
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        set_admin(&env, &admin);
        set_paused(&env, false);
    }

    /// Upgrade the contract's WASM code. Only admin can call.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        let admin = get_admin(&env);
        admin.require_auth();
        // Record upgrade history before performing upgrade
        push_upgrade_history(&env, &new_wasm_hash);
        // Perform the protocol-level upgrade
        env.deployer().update_current_contract_wasm(new_wasm_hash);
    }

    /// Pause contract functionality (emergency stop).
    pub fn pause(env: Env) {
        let admin = get_admin(&env);
        admin.require_auth();
        set_paused(&env, true);
    }

    /// Unpause contract functionality.
    pub fn unpause(env: Env) {
        let admin = get_admin(&env);
        admin.require_auth();
        set_paused(&env, false);
    }

    /// Returns the admin address.
    pub fn admin(env: Env) -> Address {
        get_admin(&env)
    }

    /// Returns whether the contract is paused.
    pub fn paused(env: Env) -> bool {
        is_paused(&env)
    }

    /// Returns the list of previously applied WASM hashes.
    pub fn upgrade_history(env: Env) -> Vec<BytesN<32>> {
        get_upgrade_history(&env)
    }

    /// Fallback called when an undefined method is invoked.
    pub fn fallback(_env: Env) {
        // In Soroban, undefined calls result in a panic.
        panic!("Method not found");
    }
}
