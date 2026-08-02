#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes, BytesN,
    Env, Symbol,
};

#[contracttype]
#[derive(Clone)]
pub struct PuzzleMeta {
    pub id: u32,
    pub solution_hash: BytesN<32>,
    pub start_ts: u64,
    pub end_ts: u64,
    pub difficulty: u32,
    pub reward_points: i128,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Puzzle(u32),
    Completed(Address, u32),
    Rewards(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Reward point arithmetic overflowed (reward_points × difficulty, or
    /// accumulated rewards). See Issue #15.
    RewardOverflow = 1,
}

#[contract]
pub struct PuzzleVerification;

#[contractimpl]
impl PuzzleVerification {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    fn require_admin(env: &Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin");
        admin.require_auth();
    }

    pub fn set_puzzle(
        env: Env,
        puzzle_id: u32,
        solution_hash: BytesN<32>,
        start_ts: u64,
        end_ts: u64,
        difficulty: u32,
        reward_points: i128,
    ) {
        Self::require_admin(&env);

        if end_ts <= start_ts {
            panic!("invalid time window");
        }

        let meta = PuzzleMeta {
            id: puzzle_id,
            solution_hash,
            start_ts,
            end_ts,
            difficulty,
            reward_points,
        };

        env.storage()
            .instance()
            .set(&DataKey::Puzzle(puzzle_id), &meta);
    }

    pub fn verify_solution(
        env: Env,
        player: Address,
        puzzle_id: u32,
        solution_preimage: Bytes,
    ) -> bool {
        player.require_auth();

        if Self::is_completed(env.clone(), player.clone(), puzzle_id) {
            panic!("puzzle already completed");
        }

        let meta: PuzzleMeta = env
            .storage()
            .instance()
            .get(&DataKey::Puzzle(puzzle_id))
            .expect("puzzle");

        let now = env.ledger().timestamp();

        if now < meta.start_ts || now > meta.end_ts {
            panic!("puzzle not active");
        }

        let computed: BytesN<32> = env.crypto().sha256(&solution_preimage).into();

        if computed != meta.solution_hash {
            return false;
        }

        env.storage()
            .instance()
            .set(&DataKey::Completed(player.clone(), puzzle_id), &true);

        let scaled = match meta
            .reward_points
            .checked_mul((meta.difficulty as i128).max(1))
        {
            Some(v) => v,
            None => panic_with_error!(&env, Error::RewardOverflow),
        };

        let rewards: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Rewards(player.clone()))
            .unwrap_or(0);

        let rewards = match rewards.checked_add(scaled) {
            Some(v) => v,
            None => panic_with_error!(&env, Error::RewardOverflow),
        };

        env.storage()
            .instance()
            .set(&DataKey::Rewards(player.clone()), &rewards);

        env.events().publish(
            (Symbol::new(&env, "puzzle"), Symbol::new(&env, "completed")),
            (player, puzzle_id, scaled),
        );

        true
    }

    pub fn is_completed(env: Env, player: Address, puzzle_id: u32) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Completed(player, puzzle_id))
            .unwrap_or(false)
    }

    pub fn rewards_of(env: Env, player: Address) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::Rewards(player))
            .unwrap_or(0)
    }

    pub fn get_puzzle(env: Env, puzzle_id: u32) -> Option<PuzzleMeta> {
        env.storage().instance().get(&DataKey::Puzzle(puzzle_id))
    }
}

#[cfg(test)]
mod test;
