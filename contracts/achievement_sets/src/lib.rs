#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, vec, Address, Env, String, Symbol, Vec,
};

// ──────────────────────────────────────────────────────────
// TYPES
// ──────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetTier {
    Common = 0,
    Rare = 1,
    Epic = 2,
    Legendary = 3,
    Mythic = 4,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Config {
    pub admin: Address,
    pub achievement_nft: Address,
    pub reward_token: Address,
    pub max_top_entries: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct AchievementSet {
    pub id: u32,
    pub name: String,
    pub required_puzzle_ids: Vec<u32>,
    pub tier: SetTier,
    pub base_bonus: i128,
    pub limited_edition_cap: Option<u32>,
    pub unlock_key: Symbol,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Synergy {
    pub id: u32,
    pub name: String,
    pub required_set_ids: Vec<u32>,
    pub bonus: i128,
    pub unlock_key: Symbol,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct SetProgressView {
    pub completed_puzzle_ids: Vec<u32>,
    pub required_count: u32,
    pub completed_count: u32,
    pub is_completed: bool,
    pub is_claimed: bool,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct SetLeaderboardEntry {
    pub player: Address,
    pub score: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct EditionToken {
    pub token_id: u32,
    pub set_id: u32,
    pub serial: u32,
    pub owner: Address,
}

// ──────────────────────────────────────────────────────────
// STORAGE
// ──────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Config,
    NextSetId,
    NextSynergyId,
    Set(u32),
    Synergy(u32),
    PlayerProgress(Address, u32),
    SetClaimed(Address, u32),
    SynergyClaimed(Address, u32),
    SetClaims(u32),
    Unlocks(Address),
    SetLeaderboard(u32),
    GlobalLeaderboard,
    PlayerTotalBonus(Address),
    NextEditionTokenId,
    EditionToken(u32),
    EditionByOwner(Address),
}

// ──────────────────────────────────────────────────────────
// EVENTS
// ──────────────────────────────────────────────────────────

const EVT_SET_CLAIM: Symbol = symbol_short!("setclaim");
const EVT_SYN_CLAIM: Symbol = symbol_short!("synclaim");
const EVT_ED_MINT: Symbol = symbol_short!("edmint");
const EVT_ED_XFER: Symbol = symbol_short!("edxfer");

// ──────────────────────────────────────────────────────────
// EXTERNAL CLIENTS
// ──────────────────────────────────────────────────────────

#[soroban_sdk::contractclient(name = "AchievementNFTClient")]
pub trait AchievementNFT {
    fn puzzle_ids_of(env: Env, owner: Address) -> Vec<u32>;
}

#[soroban_sdk::contractclient(name = "RewardTokenClient")]
pub trait RewardToken {
    fn mint(env: Env, minter: Address, to: Address, amount: i128);
}

// ──────────────────────────────────────────────────────────
// CONTRACT
// ──────────────────────────────────────────────────────────

#[contract]
pub struct AchievementSets;

#[contractimpl]
impl AchievementSets {
    pub fn initialize(
        env: Env,
        admin: Address,
        achievement_nft: Address,
        reward_token: Address,
        max_top_entries: u32,
    ) {
        admin.require_auth();

        if env.storage().persistent().has(&DataKey::Config) {
            panic!("Already initialized");
        }
        if max_top_entries == 0 {
            panic!("max_top_entries must be positive");
        }

        let cfg = Config {
            admin,
            achievement_nft,
            reward_token,
            max_top_entries,
        };

        env.storage().persistent().set(&DataKey::Config, &cfg);
        env.storage().persistent().set(&DataKey::NextSetId, &1u32);
        env.storage().persistent().set(&DataKey::NextSynergyId, &1u32);
        env.storage()
            .persistent()
            .set(&DataKey::NextEditionTokenId, &1u32);
    }

    // ───────────── Admin: define sets/synergies ─────────────

    pub fn create_set(
        env: Env,
        admin: Address,
        name: String,
        required_puzzle_ids: Vec<u32>,
        tier: SetTier,
        base_bonus: i128,
        limited_edition_cap: Option<u32>,
        unlock_key: Symbol,
    ) -> u32 {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if required_puzzle_ids.len() == 0 {
            panic!("empty set");
        }
        if base_bonus <= 0 {
            panic!("base_bonus must be positive");
        }
        if let Some(cap) = limited_edition_cap {
            if cap == 0 {
                panic!("cap must be positive");
            }
        }

        let id: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::NextSetId)
            .unwrap_or(1);

        let set = AchievementSet {
            id,
            name,
            required_puzzle_ids,
            tier,
            base_bonus,
            limited_edition_cap,
            unlock_key,
        };

        env.storage().persistent().set(&DataKey::Set(id), &set);
        env.storage()
            .persistent()
            .set(&DataKey::NextSetId, &(id + 1));

        id
    }

    pub fn create_synergy(
        env: Env,
        admin: Address,
        name: String,
        required_set_ids: Vec<u32>,
        bonus: i128,
        unlock_key: Symbol,
    ) -> u32 {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if required_set_ids.len() < 2 {
            panic!("synergy needs >=2 sets");
        }
        if bonus <= 0 {
            panic!("bonus must be positive");
        }

        for sid in required_set_ids.iter() {
            let set_id = sid.clone();
            Self::load_set(&env, set_id);
        }

        let id: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::NextSynergyId)
            .unwrap_or(1);

        let syn = Synergy {
            id,
            name,
            required_set_ids,
            bonus,
            unlock_key,
        };

        env.storage().persistent().set(&DataKey::Synergy(id), &syn);
        env.storage()
            .persistent()
            .set(&DataKey::NextSynergyId, &(id + 1));

        id
    }

    // ───────────── Views ─────────────

    pub fn get_config(env: Env) -> Config {
        env.storage().persistent().get(&DataKey::Config).unwrap()
    }

    pub fn get_set(env: Env, set_id: u32) -> Option<AchievementSet> {
        env.storage().persistent().get(&DataKey::Set(set_id))
    }

    pub fn get_synergy(env: Env, synergy_id: u32) -> Option<Synergy> {
        env.storage().persistent().get(&DataKey::Synergy(synergy_id))
    }

    pub fn get_unlocks(env: Env, player: Address) -> Vec<Symbol> {
        env.storage()
            .persistent()
            .get(&DataKey::Unlocks(player))
            .unwrap_or(Vec::new(&env))
    }

    pub fn progress(env: Env, player: Address, set_id: u32) -> SetProgressView {
        let set = Self::load_set(&env, set_id);
        let completed: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerProgress(player.clone(), set_id))
            .unwrap_or(Vec::new(&env));
        let is_claimed: bool = env
            .storage()
            .persistent()
            .get(&DataKey::SetClaimed(player, set_id))
            .unwrap_or(false);

        let required_count = set.required_puzzle_ids.len();
        let completed_count = completed.len();
        let is_completed =
            completed_count >= required_count && Self::is_completed(&set, &completed);

        SetProgressView {
            completed_puzzle_ids: completed,
            required_count: required_count as u32,
            completed_count: completed_count as u32,
            is_completed,
            is_claimed,
        }
    }

    pub fn get_set_leaderboard(env: Env, set_id: u32, limit: u32) -> Vec<SetLeaderboardEntry> {
        let cfg = Self::load_config(&env);
        let lb: Vec<SetLeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::SetLeaderboard(set_id))
            .unwrap_or(Vec::new(&env));

        let actual = if limit == 0 {
            0
        } else if limit > cfg.max_top_entries {
            cfg.max_top_entries
        } else {
            limit
        };

        let mut out = Vec::new(&env);
        for i in 0..lb.len().min(actual) {
            out.push_back(lb.get(i).unwrap());
        }
        out
    }

    pub fn get_global_leaderboard(env: Env, limit: u32) -> Vec<SetLeaderboardEntry> {
        let cfg = Self::load_config(&env);
        let lb: Vec<SetLeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::GlobalLeaderboard)
            .unwrap_or(Vec::new(&env));

        let actual = if limit == 0 {
            0
        } else if limit > cfg.max_top_entries {
            cfg.max_top_entries
        } else {
            limit
        };

        let mut out = Vec::new(&env);
        for i in 0..lb.len().min(actual) {
            out.push_back(lb.get(i).unwrap());
        }
        out
    }

    pub fn edition_tokens_of(env: Env, owner: Address) -> Vec<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::EditionByOwner(owner))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_edition_token(env: Env, token_id: u32) -> Option<EditionToken> {
        env.storage().persistent().get(&DataKey::EditionToken(token_id))
    }

    // ───────────── Core: sync + claim ─────────────

    pub fn sync_player_set(env: Env, player: Address, set_id: u32) -> SetProgressView {
        let set = Self::load_set(&env, set_id);
        let cfg = Self::load_config(&env);

        let nft = AchievementNFTClient::new(&env, &cfg.achievement_nft);
        let owned_puzzles = nft.puzzle_ids_of(&player);

        let mut completed = Vec::new(&env);
        for pid in set.required_puzzle_ids.iter() {
            let puzzle_id = pid.clone();
            if owned_puzzles.contains(&puzzle_id) && !completed.contains(&puzzle_id) {
                completed.push_back(puzzle_id);
            }
        }

        env.storage()
            .persistent()
            .set(&DataKey::PlayerProgress(player.clone(), set_id), &completed);

        Self::progress(env, player, set_id)
    }

    pub fn claim_set_bonus(env: Env, player: Address, set_id: u32) -> i128 {
        player.require_auth();

        let set = Self::load_set(&env, set_id);
        let cfg = Self::load_config(&env);

        let already: bool = env
            .storage()
            .persistent()
            .get(&DataKey::SetClaimed(player.clone(), set_id))
            .unwrap_or(false);
        if already {
            panic!("already claimed");
        }

        if let Some(cap) = set.limited_edition_cap {
            let claimed: u32 = env
                .storage()
                .persistent()
                .get(&DataKey::SetClaims(set_id))
                .unwrap_or(0);
            if claimed >= cap {
                panic!("Limited edition exhausted");
            }
        }

        let view = Self::sync_player_set(env.clone(), player.clone(), set_id);
        if !view.is_completed {
            panic!("not completed");
        }

        let tier_bonus = Self::tier_bonus(set.tier);
        let bonus = set.base_bonus + tier_bonus;

        let token = RewardTokenClient::new(&env, &cfg.reward_token);
        let minter = env.current_contract_address();

        // Fix 1: vec![] requires soroban_sdk::vec import (added to use block at top)
        env.authorize_as_current_contract(vec![&env]);
        token.mint(&minter, &player, &bonus);

        env.storage()
            .persistent()
            .set(&DataKey::SetClaimed(player.clone(), set_id), &true);

        if let Some(_cap) = set.limited_edition_cap {
            let claimed: u32 = env
                .storage()
                .persistent()
                .get(&DataKey::SetClaims(set_id))
                .unwrap_or(0);
            let new_claimed = claimed + 1;
            env.storage()
                .persistent()
                .set(&DataKey::SetClaims(set_id), &new_claimed);

            let token_id: u32 = env
                .storage()
                .persistent()
                .get(&DataKey::NextEditionTokenId)
                .unwrap_or(1);
            env.storage()
                .persistent()
                .set(&DataKey::NextEditionTokenId, &(token_id + 1));

            let ed = EditionToken {
                token_id,
                set_id,
                serial: new_claimed,
                owner: player.clone(),
            };

            env.storage()
                .persistent()
                .set(&DataKey::EditionToken(token_id), &ed);

            let mut owned: Vec<u32> = env
                .storage()
                .persistent()
                .get(&DataKey::EditionByOwner(player.clone()))
                .unwrap_or(Vec::new(&env));
            owned.push_back(token_id);
            env.storage()
                .persistent()
                .set(&DataKey::EditionByOwner(player.clone()), &owned);

            env.events()
                .publish((EVT_ED_MINT, player.clone()), (set_id, token_id, new_claimed));
        }

        Self::grant_unlock(&env, &player, set.unlock_key);
        Self::add_player_bonus(&env, &cfg, &player, bonus);
        Self::update_set_leaderboard(&env, &cfg, set_id, &player, bonus);

        env.events()
            .publish((EVT_SET_CLAIM, player.clone()), (set_id, bonus));

        bonus
    }

    pub fn claim_synergy_bonus(env: Env, player: Address, synergy_id: u32) -> i128 {
        player.require_auth();

        let cfg = Self::load_config(&env);
        let syn = Self::load_synergy(&env, synergy_id);

        let already: bool = env
            .storage()
            .persistent()
            .get(&DataKey::SynergyClaimed(player.clone(), synergy_id))
            .unwrap_or(false);
        if already {
            panic!("already claimed");
        }

        for sid in syn.required_set_ids.iter() {
            let set_id = sid.clone();
            let view = Self::sync_player_set(env.clone(), player.clone(), set_id);
            if !view.is_completed {
                panic!("synergy not completed");
            }
        }

        let token = RewardTokenClient::new(&env, &cfg.reward_token);
        let minter = env.current_contract_address();

        // Fix 1 (same): vec![] → vec![&env]
        env.authorize_as_current_contract(vec![&env]);
        token.mint(&minter, &player, &syn.bonus);

        env.storage()
            .persistent()
            .set(&DataKey::SynergyClaimed(player.clone(), synergy_id), &true);

        Self::grant_unlock(&env, &player, syn.unlock_key);
        Self::add_player_bonus(&env, &cfg, &player, syn.bonus);
        Self::update_global_leaderboard(&env, &cfg, &player);

        env.events()
            .publish((EVT_SYN_CLAIM, player.clone()), (synergy_id, syn.bonus));

        syn.bonus
    }

    // ───────────── Trading: limited edition tokens ─────────────

    pub fn transfer_edition_token(env: Env, from: Address, to: Address, token_id: u32) {
        from.require_auth();

        if from == to {
            panic!("Cannot transfer to self");
        }

        let mut token: EditionToken = env
            .storage()
            .persistent()
            .get(&DataKey::EditionToken(token_id))
            .expect("token");

        if token.owner != from {
            panic!("Not the owner");
        }

        let mut from_list: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::EditionByOwner(from.clone()))
            .unwrap_or(Vec::new(&env));
        if let Some(idx) = from_list.first_index_of(token_id) {
            from_list.remove(idx);
        }
        env.storage()
            .persistent()
            .set(&DataKey::EditionByOwner(from.clone()), &from_list);

        let mut to_list: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::EditionByOwner(to.clone()))
            .unwrap_or(Vec::new(&env));
        if !to_list.contains(&token_id) {
            to_list.push_back(token_id);
        }
        env.storage()
            .persistent()
            .set(&DataKey::EditionByOwner(to.clone()), &to_list);

        token.owner = to.clone();
        env.storage()
            .persistent()
            .set(&DataKey::EditionToken(token_id), &token);

        env.events()
            .publish((EVT_ED_XFER, from, to), (token_id, token.set_id));
    }

    // ──────────────────────────────────────────────────────────
    // INTERNALS
    // ──────────────────────────────────────────────────────────

    fn load_config(env: &Env) -> Config {
        env.storage().persistent().get(&DataKey::Config).unwrap()
    }

    fn load_set(env: &Env, set_id: u32) -> AchievementSet {
        env.storage()
            .persistent()
            .get(&DataKey::Set(set_id))
            .expect("set")
    }

    fn load_synergy(env: &Env, synergy_id: u32) -> Synergy {
        env.storage()
            .persistent()
            .get(&DataKey::Synergy(synergy_id))
            .expect("synergy")
    }

    fn assert_admin(env: &Env, caller: &Address) {
        let cfg: Config = env.storage().persistent().get(&DataKey::Config).unwrap();
        if cfg.admin != *caller {
            panic!("Admin only");
        }
    }

    fn is_completed(set: &AchievementSet, completed_puzzle_ids: &Vec<u32>) -> bool {
        if completed_puzzle_ids.len() < set.required_puzzle_ids.len() {
            return false;
        }
        for pid in set.required_puzzle_ids.iter() {
            let puzzle_id = pid.clone();
            if !completed_puzzle_ids.contains(&puzzle_id) {
                return false;
            }
        }
        true
    }

    fn tier_bonus(tier: SetTier) -> i128 {
        match tier {
            SetTier::Common => 0,
            SetTier::Rare => 50,
            SetTier::Epic => 125,
            SetTier::Legendary => 300,
            SetTier::Mythic => 750,
        }
    }

    fn grant_unlock(env: &Env, player: &Address, unlock: Symbol) {
        let mut unlocks: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&DataKey::Unlocks(player.clone()))
            .unwrap_or(Vec::new(env));
        if !unlocks.contains(&unlock) {
            unlocks.push_back(unlock);
            env.storage()
                .persistent()
                .set(&DataKey::Unlocks(player.clone()), &unlocks);
        }
    }

    fn add_player_bonus(env: &Env, cfg: &Config, player: &Address, delta: i128) {
        let mut total: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerTotalBonus(player.clone()))
            .unwrap_or(0);
        total += delta;
        env.storage()
            .persistent()
            .set(&DataKey::PlayerTotalBonus(player.clone()), &total);

        Self::update_global_leaderboard(env, cfg, player);
    }

    fn update_set_leaderboard(
        env: &Env,
        cfg: &Config,
        set_id: u32,
        player: &Address,
        score: i128,
    ) {
        let now = env.ledger().timestamp();
        // Fix 2: `let mut lb` — variable doesn't need to be mutable; use directly
        let lb: Vec<SetLeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::SetLeaderboard(set_id))
            .unwrap_or(Vec::new(env));

        let mut filtered = Vec::new(env);
        for e in lb.iter() {
            if e.player != *player {
                filtered.push_back(e);
            }
        }

        let entry = SetLeaderboardEntry {
            player: player.clone(),
            score,
            timestamp: now,
        };

        let mut out = Vec::new(env);
        let mut inserted = false;
        for e in filtered.iter() {
            if !inserted && entry.score > e.score {
                out.push_back(entry.clone());
                inserted = true;
            }
            if out.len() < cfg.max_top_entries {
                out.push_back(e);
            }
        }
        if !inserted && out.len() < cfg.max_top_entries {
            out.push_back(entry);
        }

        env.storage()
            .persistent()
            .set(&DataKey::SetLeaderboard(set_id), &out);
    }

    fn update_global_leaderboard(env: &Env, cfg: &Config, player: &Address) {
        let now = env.ledger().timestamp();
        let total: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerTotalBonus(player.clone()))
            .unwrap_or(0);

        // Fix 2: same — not mut
        let lb: Vec<SetLeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::GlobalLeaderboard)
            .unwrap_or(Vec::new(env));

        let mut filtered = Vec::new(env);
        for e in lb.iter() {
            if e.player != *player {
                filtered.push_back(e);
            }
        }

        let entry = SetLeaderboardEntry {
            player: player.clone(),
            score: total,
            timestamp: now,
        };

        let mut out = Vec::new(env);
        let mut inserted = false;
        for e in filtered.iter() {
            if !inserted && entry.score > e.score {
                out.push_back(entry.clone());
                inserted = true;
            }
            if out.len() < cfg.max_top_entries {
                out.push_back(e);
            }
        }
        if !inserted && out.len() < cfg.max_top_entries {
            out.push_back(entry);
        }

        env.storage()
            .persistent()
            .set(&DataKey::GlobalLeaderboard, &out);
    }
}

mod test;