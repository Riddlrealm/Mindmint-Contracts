#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, Vec,
};

//
// ──────────────────────────────────────────────────────────
// DATA KEYS
// ──────────────────────────────────────────────────────────
//

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Config,
    RewardPool,
    TotalStaked,
    CurrentEpochId,
    Epoch(u32),
    Stake(Address),
    StakersList,
    PlayerEpoch(u32, Address),
    ClaimedEpoch(u32, Address),
    StakeSnapshot(u32, Address),
}

//
// ──────────────────────────────────────────────────────────
// STRUCTS (issue #148)
// ──────────────────────────────────────────────────────────
//

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StakePosition {
    pub staker: Address,
    pub amount: i128,
    pub staked_at: u64,
    pub last_claim_epoch: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Epoch {
    pub epoch_id: u32,
    pub start_at: u64,
    pub end_at: u64,
    /// Sum of weighted solve credits (difficulty weights) recorded this epoch.
    pub total_solves: i128,
    /// Total tokens allocated for this epoch when closed (solver + staker pools).
    pub reward_budget: i128,
    pub distributed: bool,
    /// `total_staked` snapshot at epoch close (for staker reward split).
    pub total_staked_snapshot: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolConfig {
    pub admin: Address,
    pub oracle: Address,
    pub token: Address,
    pub epoch_duration_secs: u64,
    pub unstake_lock_secs: u64,
    /// Portion of each epoch reward budget for solvers (basis points).
    pub solver_share_bps: u32,
}

const BASIS_POINTS: i128 = 10_000;

//
// ──────────────────────────────────────────────────────────
// CONTRACT
// ──────────────────────────────────────────────────────────
//

#[contract]
pub struct PuzzlePoolStakingContract;

#[contractimpl]
impl PuzzlePoolStakingContract {
    /// Initialize puzzle reward pool staking: admin, oracle, token, epoch length, unstake lock, solver share.
    pub fn initialize(
        env: Env,
        admin: Address,
        oracle: Address,
        token: Address,
        epoch_duration_secs: u64,
        unstake_lock_secs: u64,
        solver_share_bps: u32,
    ) {
        admin.require_auth();

        if env.storage().instance().has(&DataKey::Config) {
            panic!("Already initialized");
        }
        if solver_share_bps > 10_000 {
            panic!("Invalid solver share");
        }

        let now = env.ledger().timestamp();
        let cfg = PoolConfig {
            admin,
            oracle,
            token: token.clone(),
            epoch_duration_secs,
            unstake_lock_secs,
            solver_share_bps,
        };

        env.storage().instance().set(&DataKey::Config, &cfg);
        env.storage().instance().set(&DataKey::RewardPool, &0i128);
        env.storage().instance().set(&DataKey::TotalStaked, &0i128);
        env.storage().instance().set(&DataKey::CurrentEpochId, &0u32);

        let epoch0 = Epoch {
            epoch_id: 0,
            start_at: now,
            end_at: now.saturating_add(epoch_duration_secs),
            total_solves: 0,
            reward_budget: 0,
            distributed: false,
            total_staked_snapshot: 0,
        };
        env.storage().persistent().set(&DataKey::Epoch(0), &epoch0);

        let stakers: Vec<Address> = Vec::new(&env);
        env.storage().persistent().set(&DataKey::StakersList, &stakers);
    }

    /// Admin adds reward tokens to the pool (used when closing epochs).
    pub fn fund_pool(env: Env, admin: Address, amount: i128) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        if amount <= 0 {
            panic!("Amount must be positive");
        }

        let cfg: PoolConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        let token_client = token::Client::new(&env, &cfg.token);
        token_client.transfer(&admin, &env.current_contract_address(), &amount);

        let pool: i128 = env.storage().instance().get(&DataKey::RewardPool).unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::RewardPool, &(pool + amount));
    }

    /// Deposit stake into the shared pool.
    pub fn stake(env: Env, staker: Address, amount: i128) {
        staker.require_auth();
        if amount <= 0 {
            panic!("Amount must be positive");
        }

        let cfg: PoolConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        let token_client = token::Client::new(&env, &cfg.token);
        token_client.transfer(&staker, &env.current_contract_address(), &amount);

        let now = env.ledger().timestamp();
        let mut pos = Self::get_stake_position_internal(&env, &staker).unwrap_or(StakePosition {
            staker: staker.clone(),
            amount: 0,
            staked_at: now,
            last_claim_epoch: 0,
        });

        pos.amount += amount;
        pos.staked_at = now;
        pos.staker = staker.clone();

        env.storage().persistent().set(&DataKey::Stake(staker.clone()), &pos);

        let total: i128 = env.storage().instance().get(&DataKey::TotalStaked).unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalStaked, &(total + amount));

        Self::add_to_stakers_list(&env, staker.clone());

        env.events().publish((symbol_short!("staked"), staker.clone()), amount);
    }

    /// Withdraw stake after lock; partial unstake allowed once unlocked.
    pub fn unstake(env: Env, staker: Address, amount: i128) {
        staker.require_auth();
        if amount <= 0 {
            panic!("Amount must be positive");
        }

        let cfg: PoolConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        let mut pos: StakePosition = env
            .storage()
            .persistent()
            .get(&DataKey::Stake(staker.clone()))
            .expect("No stake");

        if pos.amount < amount {
            panic!("Insufficient stake");
        }

        let now = env.ledger().timestamp();
        let elapsed = now.saturating_sub(pos.staked_at);
        if elapsed < cfg.unstake_lock_secs {
            panic!("Stake still locked");
        }

        pos.amount -= amount;
        env.storage().persistent().set(&DataKey::Stake(staker.clone()), &pos);

        let total: i128 = env.storage().instance().get(&DataKey::TotalStaked).unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalStaked, &(total - amount));

        if pos.amount == 0 {
            env.storage().persistent().remove(&DataKey::Stake(staker.clone()));
            Self::remove_from_stakers_list(&env, staker.clone());
        }

        let token_client = token::Client::new(&env, &cfg.token);
        token_client.transfer(&env.current_contract_address(), &staker, &amount);

        env.events()
            .publish((symbol_short!("unstkd"), staker.clone()), amount);
    }

    /// Oracle records a weighted solve for the current epoch (`difficulty` is the weight).
    pub fn record_solve(env: Env, oracle: Address, player: Address, puzzle_difficulty: u32) {
        oracle.require_auth();
        let cfg: PoolConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        if cfg.oracle != oracle {
            panic!("Oracle only");
        }
        if puzzle_difficulty == 0 {
            panic!("Invalid difficulty");
        }

        let current: u32 = env.storage().instance().get(&DataKey::CurrentEpochId).unwrap();
        let mut epoch: Epoch = env
            .storage()
            .persistent()
            .get(&DataKey::Epoch(current))
            .expect("Epoch");
        if epoch.distributed {
            panic!("Epoch closed");
        }

        let w = puzzle_difficulty as i128;
        let key = DataKey::PlayerEpoch(current, player.clone());
        let prev: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(prev + w));

        epoch.total_solves += w;
        env.storage().persistent().set(&DataKey::Epoch(current), &epoch);
    }

    /// Admin closes the current epoch, commits `reward_budget` from the reward pool, snapshots stakes, opens next epoch.
    pub fn close_epoch(env: Env, admin: Address, reward_budget: i128) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        if reward_budget <= 0 {
            panic!("Invalid reward budget");
        }

        let pool: i128 = env.storage().instance().get(&DataKey::RewardPool).unwrap_or(0);
        if pool < reward_budget {
            panic!("Insufficient reward pool");
        }

        let cfg: PoolConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        let current: u32 = env.storage().instance().get(&DataKey::CurrentEpochId).unwrap();
        let mut epoch: Epoch = env
            .storage()
            .persistent()
            .get(&DataKey::Epoch(current))
            .expect("Epoch");
        if epoch.distributed {
            panic!("Already closed");
        }

        let now = env.ledger().timestamp();
        if now < epoch.end_at {
            panic!("Epoch not ended");
        }

        let total_staked: i128 = env.storage().instance().get(&DataKey::TotalStaked).unwrap_or(0);

        // Snapshot per-staker amounts for this epoch's staker rewards.
        let stakers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::StakersList)
            .unwrap_or(Vec::new(&env));
        let n = stakers.len();
        let mut i = 0u32;
        while i < n {
            let staker = stakers.get(i).unwrap();
            if let Some(p) = env
                .storage()
                .persistent()
                .get::<DataKey, StakePosition>(&DataKey::Stake(staker.clone()))
            {
                if p.amount > 0 {
                    env.storage().persistent().set(
                        &DataKey::StakeSnapshot(current, staker.clone()),
                        &p.amount,
                    );
                }
            }
            i += 1;
        }

        epoch.reward_budget = reward_budget;
        epoch.distributed = true;
        epoch.total_staked_snapshot = total_staked;
        env.storage().persistent().set(&DataKey::Epoch(current), &epoch);

        env.storage()
            .instance()
            .set(&DataKey::RewardPool, &(pool - reward_budget));

        let next_id = current + 1;
        let next = Epoch {
            epoch_id: next_id,
            start_at: now,
            end_at: now.saturating_add(cfg.epoch_duration_secs),
            total_solves: 0,
            reward_budget: 0,
            distributed: false,
            total_staked_snapshot: 0,
        };
        env.storage().persistent().set(&DataKey::Epoch(next_id), &next);
        env.storage().instance().set(&DataKey::CurrentEpochId, &next_id);

        env.events().publish(
            (symbol_short!("epclosed"), admin.clone()),
            (current, reward_budget),
        );
    }

    /// Claim solver + staker rewards for a closed epoch (idempotent per player per epoch).
    pub fn claim_epoch_reward(env: Env, player: Address, epoch_id: u32) -> i128 {
        player.require_auth();

        let cfg: PoolConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        let epoch: Epoch = env
            .storage()
            .persistent()
            .get(&DataKey::Epoch(epoch_id))
            .expect("Epoch");
        if !epoch.distributed {
            panic!("Epoch not closed");
        }

        let claimed_key = DataKey::ClaimedEpoch(epoch_id, player.clone());
        if env.storage().persistent().has(&claimed_key) {
            panic!("Already claimed");
        }

        let player_weight: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerEpoch(epoch_id, player.clone()))
            .unwrap_or(0);

        let snapshot: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::StakeSnapshot(epoch_id, player.clone()))
            .unwrap_or(0);

        let reward_budget = epoch.reward_budget;
        let solver_bps = cfg.solver_share_bps as i128;
        let mut solver_pool = (reward_budget * solver_bps) / BASIS_POINTS;
        let mut staker_pool = reward_budget - solver_pool;

        if epoch.total_solves == 0 {
            staker_pool += solver_pool;
            solver_pool = 0;
        }
        if epoch.total_staked_snapshot == 0 {
            solver_pool += staker_pool;
            staker_pool = 0;
        }

        let mut solver_reward: i128 = 0;
        if epoch.total_solves > 0 && solver_pool > 0 {
            solver_reward = (solver_pool * player_weight) / epoch.total_solves;
        }

        let mut staker_reward: i128 = 0;
        if epoch.total_staked_snapshot > 0 && staker_pool > 0 && snapshot > 0 {
            staker_reward = (staker_pool * snapshot) / epoch.total_staked_snapshot;
        }

        let total = solver_reward + staker_reward;
        if total <= 0 {
            panic!("Nothing to claim");
        }

        env.storage().persistent().set(&claimed_key, &true);

        if let Some(mut pos) = Self::get_stake_position_internal(&env, &player) {
            if epoch_id > pos.last_claim_epoch {
                pos.last_claim_epoch = epoch_id;
                env.storage()
                    .persistent()
                    .set(&DataKey::Stake(player.clone()), &pos);
            }
        }

        let token_client = token::Client::new(&env, &cfg.token);
        token_client.transfer(&env.current_contract_address(), &player, &total);

        env.events().publish(
            (symbol_short!("clmrew"), player.clone()),
            (epoch_id, total),
        );

        total
    }

    /// View: stake position and epoch ids that are closed and not yet claimed by this player.
    pub fn get_stake(env: Env, player: Address) -> (StakePosition, Vec<u32>) {
        let pos = Self::get_stake_position_internal(&env, &player).unwrap_or(StakePosition {
            staker: player.clone(),
            amount: 0,
            staked_at: 0,
            last_claim_epoch: 0,
        });

        let current: u32 = env.storage().instance().get(&DataKey::CurrentEpochId).unwrap();
        let mut out: Vec<u32> = Vec::new(&env);

        let mut e: u32 = 0;
        while e < current {
            let ep: Epoch = env.storage().persistent().get(&DataKey::Epoch(e)).unwrap();
            if ep.distributed {
                let ck = DataKey::ClaimedEpoch(e, player.clone());
                if !env.storage().persistent().has(&ck) {
                    out.push_back(e);
                }
            }
            e += 1;
        }

        (pos, out)
    }

    pub fn get_epoch(env: Env, epoch_id: u32) -> Epoch {
        env.storage()
            .persistent()
            .get(&DataKey::Epoch(epoch_id))
            .expect("Epoch")
    }

    pub fn get_reward_pool(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::RewardPool).unwrap_or(0)
    }

    pub fn get_total_staked(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::TotalStaked).unwrap_or(0)
    }

    pub fn get_config(env: Env) -> PoolConfig {
        env.storage().instance().get(&DataKey::Config).unwrap()
    }

    pub fn get_current_epoch_id(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::CurrentEpochId).unwrap()
    }

    pub fn preview_claim(env: Env, player: Address, epoch_id: u32) -> i128 {
        let cfg: PoolConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        let epoch: Epoch = env
            .storage()
            .persistent()
            .get(&DataKey::Epoch(epoch_id))
            .expect("Epoch");
        if !epoch.distributed {
            return 0;
        }
        if env
            .storage()
            .persistent()
            .has(&DataKey::ClaimedEpoch(epoch_id, player.clone()))
        {
            return 0;
        }

        let player_weight: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerEpoch(epoch_id, player.clone()))
            .unwrap_or(0);

        let snapshot: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::StakeSnapshot(epoch_id, player.clone()))
            .unwrap_or(0);

        let reward_budget = epoch.reward_budget;
        let solver_bps = cfg.solver_share_bps as i128;
        let mut solver_pool = (reward_budget * solver_bps) / BASIS_POINTS;
        let mut staker_pool = reward_budget - solver_pool;

        if epoch.total_solves == 0 {
            staker_pool += solver_pool;
            solver_pool = 0;
        }
        if epoch.total_staked_snapshot == 0 {
            solver_pool += staker_pool;
            staker_pool = 0;
        }

        let mut solver_reward: i128 = 0;
        if epoch.total_solves > 0 && solver_pool > 0 {
            solver_reward = (solver_pool * player_weight) / epoch.total_solves;
        }

        let mut staker_reward: i128 = 0;
        if epoch.total_staked_snapshot > 0 && staker_pool > 0 && snapshot > 0 {
            staker_reward = (staker_pool * snapshot) / epoch.total_staked_snapshot;
        }

        solver_reward + staker_reward
    }

    fn get_stake_position_internal(env: &Env, staker: &Address) -> Option<StakePosition> {
        env.storage().persistent().get(&DataKey::Stake(staker.clone()))
    }

    fn assert_admin(env: &Env, addr: &Address) {
        let cfg: PoolConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        if cfg.admin != *addr {
            panic!("Admin only");
        }
    }

    fn add_to_stakers_list(env: &Env, staker: Address) {
        let mut stakers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::StakersList)
            .unwrap_or(Vec::new(env));
        if !stakers.contains(&staker) {
            stakers.push_back(staker);
            env.storage().persistent().set(&DataKey::StakersList, &stakers);
        }
    }

    fn remove_from_stakers_list(env: &Env, staker: Address) {
        let stakers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::StakersList)
            .unwrap_or(Vec::new(env));
        let mut new_stakers: Vec<Address> = Vec::new(env);
        for s in stakers.iter() {
            if s != staker {
                new_stakers.push_back(s);
            }
        }
        env.storage().persistent().set(&DataKey::StakersList, &new_stakers);
    }
}

mod test;
