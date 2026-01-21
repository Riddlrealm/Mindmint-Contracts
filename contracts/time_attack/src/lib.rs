#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scope {
    Global,
    Puzzle(u32),
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Period {
    AllTime,
    Daily,
    Weekly,
}

#[contracttype]
pub enum DataKey {
    // Admin / config
    Admin, // Address
    TopN,  // u32

    // Timing (only used if start_run/submit_run)
    RunStart(Address, u32), // u64

    // Anti-cheat
    LastSubmit(Address),    // u64
    ReplayUsed(BytesN<32>), // bool

    // Leaderboards (flexible)
    Best(Scope, Period),      // TimeRecord (later)
    Board(Scope, Period),     // Vec<TimeRecord> (later)
    LastReset(Scope, Period), // u64
}

#[contract]
pub struct TimeAttack;

#[contractimpl]
impl TimeAttack {
    pub fn initialize(env: Env, admin: Address) {
        let storage = env.storage().instance();

        // Prevent re-initialization if an admin is already set
        if storage.has(&DataKey::Admin) {
            panic!("already initialized");
        }

        // Ensure the provided admin authorizes being set as admin
        admin.require_auth();

        // Store the admin address in contract storage
        storage.set(&DataKey::Admin, &admin);
    }
}
