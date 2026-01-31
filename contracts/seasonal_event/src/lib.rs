#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, IntoVal, String, Symbol, Vec,
};

const BPS_BASE: u32 = 10_000;

#[contracttype]
#[derive(Clone)]
pub struct Config {
    pub admin: Address,
    pub leaderboard: Option<Address>,
    pub paused: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct Event {
    pub id: u64,
    pub name: String,
    pub start_time: u64,
    pub end_time: u64,
    pub reward_amount: i128,
    pub bonus_multiplier_bps: u32,
    pub nft_metadata: String,
    pub puzzle_ids: Vec<u32>,
    pub cancelled: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct EventNft {
    pub owner: Address,
    pub event_id: u64,
    pub metadata: String,
    pub minted_at: u64,
}

#[contracttype]
pub enum DataKey {
    Config,
    Event(u64),
    NextEventId,
    EventParticipant(u64, Address),
    EventPuzzleComplete(u64, Address, u32),
    EventScore(u64, Address),
    EventRewardClaimed(u64, Address),
    EventNftClaimed(u64, Address),
    EventNft(u32),
    NextNftId,
    Verifier(Address),
}

#[contract]
pub struct SeasonalEventContract;

#[contractimpl]
impl SeasonalEventContract {
    // ───────────── INITIALIZATION ─────────────

    pub fn initialize(env: Env, admin: Address, leaderboard: Option<Address>) {
        admin.require_auth();

        if env.storage().persistent().has(&DataKey::Config) {
            panic!("Already initialized");
        }

        let config = Config {
            admin,
            leaderboard,
            paused: false,
        };

        env.storage().persistent().set(&DataKey::Config, &config);
        env.storage().persistent().set(&DataKey::NextEventId, &1u64);
        env.storage().persistent().set(&DataKey::NextNftId, &1u32);
    }

    // ───────────── ADMIN FUNCTIONS ─────────────

    pub fn add_verifier(env: Env, admin: Address, verifier: Address) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        env.storage()
            .persistent()
            .set(&DataKey::Verifier(verifier), &true);
    }

    pub fn remove_verifier(env: Env, admin: Address, verifier: Address) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        env.storage().persistent().remove(&DataKey::Verifier(verifier));
    }

    pub fn set_paused(env: Env, admin: Address, paused: bool) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut config: Config = env.storage().persistent().get(&DataKey::Config).unwrap();
        config.paused = paused;
        env.storage().persistent().set(&DataKey::Config, &config);
    }

    pub fn set_leaderboard(env: Env, admin: Address, leaderboard: Option<Address>) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut config: Config = env.storage().persistent().get(&DataKey::Config).unwrap();
        config.leaderboard = leaderboard;
        env.storage().persistent().set(&DataKey::Config, &config);
    }

    pub fn create_event(
        env: Env,
        admin: Address,
        name: String,
        start_time: u64,
        end_time: u64,
        reward_amount: i128,
        bonus_multiplier_bps: u32,
        nft_metadata: String,
        puzzle_ids: Vec<u32>,
    ) -> u64 {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if start_time >= end_time {
            panic!("Invalid event time range");
        }

        let mut next_id: u64 = env.storage().persistent().get(&DataKey::NextEventId).unwrap();
        let bonus = if bonus_multiplier_bps == 0 {
            BPS_BASE
        } else {
            bonus_multiplier_bps
        };

        let event = Event {
            id: next_id,
            name,
            start_time,
            end_time,
            reward_amount,
            bonus_multiplier_bps: bonus,
            nft_metadata,
            puzzle_ids,
            cancelled: false,
        };

        env.storage().persistent().set(&DataKey::Event(next_id), &event);
        next_id += 1;
        env.storage().persistent().set(&DataKey::NextEventId, &next_id);

        event.id
    }

    pub fn update_event_times(
        env: Env,
        admin: Address,
        event_id: u64,
        start_time: u64,
        end_time: u64,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if start_time >= end_time {
            panic!("Invalid event time range");
        }

        let mut event = Self::get_event_internal(&env, event_id);
        event.start_time = start_time;
        event.end_time = end_time;
        env.storage().persistent().set(&DataKey::Event(event_id), &event);
    }

    pub fn update_event_rewards(
        env: Env,
        admin: Address,
        event_id: u64,
        reward_amount: i128,
        bonus_multiplier_bps: u32,
        nft_metadata: String,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut event = Self::get_event_internal(&env, event_id);
        event.reward_amount = reward_amount;
        event.bonus_multiplier_bps = if bonus_multiplier_bps == 0 {
            BPS_BASE
        } else {
            bonus_multiplier_bps
        };
        event.nft_metadata = nft_metadata;
        env.storage().persistent().set(&DataKey::Event(event_id), &event);
    }

    pub fn update_event_puzzles(
        env: Env,
        admin: Address,
        event_id: u64,
        puzzle_ids: Vec<u32>,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut event = Self::get_event_internal(&env, event_id);
        event.puzzle_ids = puzzle_ids;
        env.storage().persistent().set(&DataKey::Event(event_id), &event);
    }

    pub fn set_event_cancelled(env: Env, admin: Address, event_id: u64, cancelled: bool) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut event = Self::get_event_internal(&env, event_id);
        event.cancelled = cancelled;
        env.storage().persistent().set(&DataKey::Event(event_id), &event);
    }

    // ───────────── EVENT PARTICIPATION ─────────────

    pub fn record_puzzle_completion(
        env: Env,
        submitter: Address,
        event_id: u64,
        user: Address,
        puzzle_id: u32,
        score: i128,
    ) {
        submitter.require_auth();
        Self::assert_admin_or_verifier(&env, &submitter);
        Self::assert_not_paused(&env);
        Self::assert_event_active(&env, event_id);

        let event = Self::get_event_internal(&env, event_id);
        if !Self::puzzle_allowed(&event, puzzle_id) {
            panic!("Puzzle not part of event");
        }

        let completion_key = DataKey::EventPuzzleComplete(event_id, user.clone(), puzzle_id);
        if env.storage().persistent().has(&completion_key) {
            panic!("Puzzle already completed");
        }

        env.storage().persistent().set(&completion_key, &true);
        env.storage()
            .persistent()
            .set(&DataKey::EventParticipant(event_id, user.clone()), &true);

        let score_key = DataKey::EventScore(event_id, user.clone());
        let prev_score: i128 = env.storage().persistent().get(&score_key).unwrap_or(0);
        let new_score = prev_score + score;
        env.storage().persistent().set(&score_key, &new_score);

        Self::submit_leaderboard_score(&env, &user, new_score);
    }

    pub fn claim_event_reward(env: Env, event_id: u64, user: Address) -> i128 {
        user.require_auth();
        Self::assert_not_paused(&env);
        Self::assert_event_active(&env, event_id);

        Self::assert_participant(&env, event_id, &user);

        let claim_key = DataKey::EventRewardClaimed(event_id, user.clone());
        if env.storage().persistent().has(&claim_key) {
            panic!("Reward already claimed");
        }

        let event = Self::get_event_internal(&env, event_id);
        let reward = Self::apply_bonus(event.reward_amount, event.bonus_multiplier_bps);

        env.storage().persistent().set(&claim_key, &true);
        env.events().publish((symbol_short!("reward"), event_id, user.clone()), reward);

        reward
    }

    pub fn mint_event_nft(env: Env, event_id: u64, user: Address) -> u32 {
        user.require_auth();
        Self::assert_not_paused(&env);
        Self::assert_event_active(&env, event_id);
        Self::assert_participant(&env, event_id, &user);

        let reward_key = DataKey::EventRewardClaimed(event_id, user.clone());
        if !env.storage().persistent().has(&reward_key) {
            panic!("Claim reward before minting");
        }

        let minted_key = DataKey::EventNftClaimed(event_id, user.clone());
        if env.storage().persistent().has(&minted_key) {
            panic!("NFT already minted");
        }

        let next_id: u32 = env.storage().persistent().get(&DataKey::NextNftId).unwrap();
        let event = Self::get_event_internal(&env, event_id);
        let nft = EventNft {
            owner: user.clone(),
            event_id,
            metadata: event.nft_metadata,
            minted_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&DataKey::EventNft(next_id), &nft);
        env.storage().persistent().set(&minted_key, &true);
        env.storage().persistent().set(&DataKey::NextNftId, &(next_id + 1));

        env.events().publish((symbol_short!("mint"), event_id, user), next_id);

        next_id
    }

    // ───────────── READ FUNCTIONS ─────────────

    pub fn get_event(env: Env, event_id: u64) -> Event {
        Self::get_event_internal(&env, event_id)
    }

    pub fn get_event_nft(env: Env, token_id: u32) -> Option<EventNft> {
        env.storage().persistent().get(&DataKey::EventNft(token_id))
    }

    pub fn get_event_score(env: Env, event_id: u64, user: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::EventScore(event_id, user))
            .unwrap_or(0)
    }

    pub fn has_completed_puzzle(env: Env, event_id: u64, user: Address, puzzle_id: u32) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::EventPuzzleComplete(event_id, user, puzzle_id))
    }

    pub fn is_event_active(env: Env, event_id: u64) -> bool {
        let event = Self::get_event_internal(&env, event_id);
        Self::is_event_active_at(&env, &event)
    }

    pub fn can_access_event_content(env: Env, event_id: u64, user: Address) -> bool {
        if !Self::is_event_active(env.clone(), event_id) {
            return false;
        }

        env.storage()
            .persistent()
            .has(&DataKey::EventParticipant(event_id, user))
    }

    // ───────────── HELPERS ─────────────

    fn assert_admin(env: &Env, admin: &Address) {
        let config: Config = env.storage().persistent().get(&DataKey::Config).unwrap();
        if &config.admin != admin {
            panic!("Unauthorized");
        }
    }

    fn assert_admin_or_verifier(env: &Env, submitter: &Address) {
        let config: Config = env.storage().persistent().get(&DataKey::Config).unwrap();
        if &config.admin == submitter {
            return;
        }
        let is_verifier = env
            .storage()
            .persistent()
            .get(&DataKey::Verifier(submitter.clone()))
            .unwrap_or(false);
        if !is_verifier {
            panic!("Unauthorized");
        }
    }

    fn assert_not_paused(env: &Env) {
        let config: Config = env.storage().persistent().get(&DataKey::Config).unwrap();
        if config.paused {
            panic!("Contract paused");
        }
    }

    fn assert_event_active(env: &Env, event_id: u64) {
        let event = Self::get_event_internal(env, event_id);
        if !Self::is_event_active_at(env, &event) {
            panic!("Event not active");
        }
    }

    fn assert_participant(env: &Env, event_id: u64, user: &Address) {
        let key = DataKey::EventParticipant(event_id, user.clone());
        if !env.storage().persistent().has(&key) {
            panic!("Not a participant");
        }
    }

    fn get_event_internal(env: &Env, event_id: u64) -> Event {
        env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .expect("Event not found")
    }

    fn is_event_active_at(env: &Env, event: &Event) -> bool {
        if event.cancelled {
            return false;
        }
        let now = env.ledger().timestamp();
        now >= event.start_time && now <= event.end_time
    }

    fn puzzle_allowed(event: &Event, puzzle_id: u32) -> bool {
        let len = event.puzzle_ids.len();
        let mut i = 0;
        while i < len {
            if event.puzzle_ids.get(i).unwrap_or(0) == puzzle_id {
                return true;
            }
            i += 1;
        }
        false
    }

    fn apply_bonus(amount: i128, bonus_bps: u32) -> i128 {
        let bonus = if bonus_bps == 0 { BPS_BASE } else { bonus_bps };
        amount * (bonus as i128) / (BPS_BASE as i128)
    }

    fn submit_leaderboard_score(env: &Env, user: &Address, score: i128) {
        let config: Config = env.storage().persistent().get(&DataKey::Config).unwrap();
        if let Some(leaderboard) = config.leaderboard {
            let func = Symbol::new(env, "submit_score");
            let mut args: Vec<soroban_sdk::Val> = Vec::new(env);
            args.push_back(env.current_contract_address().into_val(env));
            args.push_back(user.clone().into_val(env));
            args.push_back(score.into_val(env));
            env.invoke_contract::<()>(
                &leaderboard,
                &func,
                args,
            );
        }
    }
}

mod test;
