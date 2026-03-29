#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol,
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

        let scaled = meta.reward_points * (meta.difficulty as i128).max(1);

        let mut rewards: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Rewards(player.clone()))
            .unwrap_or(0);

        rewards += scaled;

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
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::testutils::Ledger as _;

    #[test]
    fn test_verification_flow() {
        let env = Env::default();
        let contract_id = env.register_contract(None, PuzzleVerification);
        let client = PuzzleVerificationClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let player = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&admin);

        env.ledger().set_timestamp(1_000);

        let preimage = Bytes::from_array(&env, &[7u8; 5]);
        let hash: BytesN<32> = env.crypto().sha256(&preimage).into();
        let now = env.ledger().timestamp();

        client.set_puzzle(&1, &hash, &(now - 1), &(now + 1000), &2, &50);

        let wrong = Bytes::from_array(&env, &[8u8; 5]);
        assert_eq!(client.verify_solution(&player, &1, &wrong), false);

        assert_eq!(client.verify_solution(&player, &1, &preimage), true);
        assert_eq!(client.is_completed(&player, &1), true);
        assert_eq!(client.rewards_of(&player), 100);
    }

    #[test]
    #[should_panic(expected = "puzzle not active")]
    fn test_expiration_enforced() {
        let env = Env::default();
        let contract_id = env.register_contract(None, PuzzleVerification);
        let client = PuzzleVerificationClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let player = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&admin);

        env.ledger().set_timestamp(1_000);

        let preimage = Bytes::from_array(&env, &[1u8; 3]);
        let hash: BytesN<32> = env.crypto().sha256(&preimage).into();
        let now = env.ledger().timestamp();

        client.set_puzzle(&42, &hash, &(now - 100), &(now - 50), &1, &10);

        let _ = client.verify_solution(&player, &42, &preimage);
    }
}