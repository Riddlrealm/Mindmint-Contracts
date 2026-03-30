#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype,
    Address, Env, Symbol, Vec
};

// ================= STRUCTS =================

#[derive(Clone)]
#[contracttype]
pub struct Jackpot {
    pub cycle_id: u64,
    pub balance: i128,
    pub target_puzzle_id: u64,
    pub deadline: u64,
    pub winner: Option<Address>,
    pub status: JackpotStatus,
}

#[derive(Clone)]
#[contracttype]
pub enum JackpotStatus {
    Active,
    Claimed,
    Expired,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Current,
    History(u64),
    Admin,
    Oracle,
}

// ================= CONTRACT =================

#[contract]
pub struct ProgressiveJackpot;

#[contractimpl]
impl ProgressiveJackpot {

    // 🔹 Initialize
    pub fn init(env: Env, admin: Address, oracle: Address) {
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Oracle, &oracle);

        let jackpot = Jackpot {
            cycle_id: 1,
            balance: 0,
            target_puzzle_id: 0,
            deadline: 0,
            winner: None,
            status: JackpotStatus::Active,
        };

        env.storage().instance().set(&DataKey::Current, &jackpot);
    }

    // 🔹 Contribute (oracle only)
    pub fn contribute(env: Env, amount: i128) {
        let oracle: Address = env.storage().instance().get(&DataKey::Oracle).unwrap();
        oracle.require_auth();

        let mut jackpot: Jackpot = env.storage().instance().get(&DataKey::Current).unwrap();
        jackpot.balance += amount;

        env.storage().instance().set(&DataKey::Current, &jackpot);

        env.events().publish(
            (Symbol::new(&env, "JackpotContributed"),),
            (amount, jackpot.balance),
        );
    }

    // 🔹 Admin sets puzzle + deadline
    pub fn set_jackpot_puzzle(env: Env, puzzle_id: u64, deadline: u64) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let mut jackpot: Jackpot = env.storage().instance().get(&DataKey::Current).unwrap();
        jackpot.target_puzzle_id = puzzle_id;
        jackpot.deadline = deadline;

        env.storage().instance().set(&DataKey::Current, &jackpot);
    }

    // 🔹 Claim jackpot
    pub fn claim_jackpot(env: Env, cycle_id: u64, player: Address) {
        player.require_auth();

        let oracle: Address = env.storage().instance().get(&DataKey::Oracle).unwrap();
        oracle.require_auth(); // proof verified off-chain

        let mut jackpot: Jackpot = env.storage().instance().get(&DataKey::Current).unwrap();

        if jackpot.cycle_id != cycle_id {
            panic!("Invalid cycle");
        }

        if let JackpotStatus::Active = jackpot.status {} else {
            panic!("Not claimable");
        }

        let now = env.ledger().timestamp();

        if now > jackpot.deadline {
            panic!("Deadline passed");
        }

        let amount = jackpot.balance;

        jackpot.balance = 0;
        jackpot.winner = Some(player.clone());
        jackpot.status = JackpotStatus::Claimed;

        // Save history
        env.storage().persistent().set(&DataKey::History(cycle_id), &jackpot);

        env.storage().instance().set(&DataKey::Current, &jackpot);

        env.events().publish(
            (Symbol::new(&env, "JackpotClaimed"),),
            (cycle_id, player, amount),
        );
    }

    // 🔹 Rollover
    pub fn rollover(env: Env) {
        let mut jackpot: Jackpot = env.storage().instance().get(&DataKey::Current).unwrap();

        let now = env.ledger().timestamp();

        if now <= jackpot.deadline {
            panic!("Deadline not reached");
        }

        if let JackpotStatus::Claimed = jackpot.status {
            panic!("Already claimed");
        }

        let old_cycle = jackpot.cycle_id;
        let balance = jackpot.balance;

        jackpot.status = JackpotStatus::Expired;

        env.storage().persistent().set(&DataKey::History(old_cycle), &jackpot);

        let new_jackpot = Jackpot {
            cycle_id: old_cycle + 1,
            balance,
            target_puzzle_id: 0,
            deadline: 0,
            winner: None,
            status: JackpotStatus::Active,
        };

        env.storage().instance().set(&DataKey::Current, &new_jackpot);

        env.events().publish(
            (Symbol::new(&env, "JackpotRolledOver"),),
            (old_cycle, old_cycle + 1, balance),
        );
    }

    // 🔹 Get current jackpot
    pub fn get_jackpot(env: Env) -> Jackpot {
        env.storage().instance().get(&DataKey::Current).unwrap()
    }

    // 🔹 Get history (single cycle)
    pub fn get_jackpot_history(env: Env, cycle_id: u64) -> Jackpot {
        env.storage().persistent().get(&DataKey::History(cycle_id)).unwrap()
    }
}