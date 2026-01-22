#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Vec};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rarity {
    Common = 0,
    Rare = 1,
    Epic = 2,
    Legendary = 3,
}

#[contracttype]
#[derive(Clone)]
pub struct Set {
    pub id: u32,
    pub name: String,
    pub achievements: Vec<u32>,
    pub rarity: Rarity,
    pub limited_cap: Option<u32>, // Total max claims per achievement across players
    pub bonus_points: i128,       // Internal bonus points rewarded on completion
}

#[contracttype]
pub enum DataKey {
    NextSetId,
    Set(u32),                // Set
    AchToSet(u32),           // u32 set_id
    PlayerProgress(Address, u32), // Vec<u32> collected achievement IDs in set
    AchCount(u32),           // u32 global claim count for limited edition
    Bonus(Address),          // i128 internal bonus ledger per player
    Completed(Address, u32), // bool marker to avoid double bonus
}

#[contract]
pub struct AchievementCollection;

#[contractimpl]
impl AchievementCollection {
    pub fn initialize(env: Env) {
        if env.storage().instance().has(&DataKey::NextSetId) {
            panic!("initialized");
        }
        env.storage().instance().set(&DataKey::NextSetId, &1u32);
    }

    // Set management
    pub fn create_set(
        env: Env,
        name: String,
        achievements: Vec<u32>,
        rarity: Rarity,
        limited_cap: Option<u32>,
        bonus_points: i128,
    ) -> u32 {
        if achievements.len() == 0 {
            panic!("empty set");
        }
        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextSetId)
            .unwrap_or(1);

        let set = Set {
            id,
            name,
            achievements: achievements.clone(),
            rarity,
            limited_cap,
            bonus_points,
        };
        env.storage().instance().set(&DataKey::Set(id), &set);
        env.storage().instance().set(&DataKey::NextSetId, &(id + 1));

        for a in achievements.iter() {
            let aid = a.clone();
            if env.storage().instance().has(&DataKey::AchToSet(aid)) {
                panic!("achievement already mapped");
            }
            env.storage().instance().set(&DataKey::AchToSet(aid), &id);
        }
        id
    }

    pub fn get_set(env: Env, set_id: u32) -> Option<Set> {
        env.storage().instance().get(&DataKey::Set(set_id))
    }

    // Map additional achievement to an existing set
    pub fn add_achievement_to_set(env: Env, set_id: u32, achievement_id: u32) {
        let mut set: Set = env
            .storage()
            .instance()
            .get(&DataKey::Set(set_id))
            .expect("set");
        if set.achievements.contains(&achievement_id) {
            return;
        }
        if env.storage().instance().has(&DataKey::AchToSet(achievement_id)) {
            panic!("achievement already mapped");
        }
        set.achievements.push_back(achievement_id);
        env.storage().instance().set(&DataKey::Set(set_id), &set);
        env.storage()
            .instance()
            .set(&DataKey::AchToSet(achievement_id), &set_id);
    }

    // Record progress and detect completion
    pub fn record_achievement(env: Env, player: Address, achievement_id: u32) -> bool {
        player.require_auth();
        let set_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::AchToSet(achievement_id))
            .expect("mapped");

        let set: Set = env
            .storage()
            .instance()
            .get(&DataKey::Set(set_id))
            .expect("set");

        // limited edition check per achievement (only when acquiring new)
        // load current progress to determine ownership first
        let progress_for_cap: Vec<u32> = env
            .storage()
            .instance()
            .get(&DataKey::PlayerProgress(player.clone(), set_id))
            .unwrap_or(Vec::new(&env.clone()));
        if let Some(cap) = set.limited_cap {
            let mut cnt: u32 = env
                .storage()
                .instance()
                .get(&DataKey::AchCount(achievement_id))
                .unwrap_or(0);
            if !progress_for_cap.contains(&achievement_id) {
                if cnt >= cap {
                    panic!("limited cap reached");
                }
                cnt += 1;
                env.storage()
                    .instance()
                    .set(&DataKey::AchCount(achievement_id), &cnt);
            }
        }

        let mut progress: Vec<u32> = env
            .storage()
            .instance()
            .get(&DataKey::PlayerProgress(player.clone(), set_id))
            .unwrap_or(Vec::new(&env.clone()));
        if !progress.contains(&achievement_id) {
            progress.push_back(achievement_id);
            env.storage()
                .instance()
                .set(&DataKey::PlayerProgress(player.clone(), set_id), &progress);
        }

        // completion detection
        let completed = Self::is_completed_internal(&set, &progress);
        if completed {
            // award bonus only once per player per set
            let already: bool = env
                .storage()
                .instance()
                .get(&DataKey::Completed(player.clone(), set_id))
                .unwrap_or(false);
            if !already {
                let mut bonus: i128 = env
                    .storage()
                    .instance()
                    .get(&DataKey::Bonus(player.clone()))
                    .unwrap_or(0);
                bonus += set.bonus_points;
                env.storage()
                    .instance()
                    .set(&DataKey::Bonus(player.clone()), &bonus);
                env.storage()
                    .instance()
                    .set(&DataKey::Completed(player.clone(), set_id), &true);
            }
        }
        completed
    }

    pub fn progress(env: Env, player: Address, set_id: u32) -> Vec<u32> {
        env.storage()
            .instance()
            .get(&DataKey::PlayerProgress(player, set_id))
            .unwrap_or(Vec::new(&env))
    }

    pub fn is_completed(env: Env, player: Address, set_id: u32) -> bool {
        let set: Set = env
            .storage()
            .instance()
            .get(&DataKey::Set(set_id))
            .expect("set");
        let progress: Vec<u32> = Self::progress(env.clone(), player, set_id);
        Self::is_completed_internal(&set, &progress)
    }

    fn is_completed_internal(set: &Set, progress: &Vec<u32>) -> bool {
        if progress.len() < set.achievements.len() {
            return false;
        }
        // every achievement in set must be present
        for a in set.achievements.iter() {
            if !progress.contains(&a) {
                return false;
            }
        }
        true
    }

    // Trading/swapping progress marks between players within a set
    pub fn transfer_progress(env: Env, from: Address, to: Address, achievement_id: u32) {
        from.require_auth();
        let set_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::AchToSet(achievement_id))
            .expect("mapped");

        let mut from_progress: Vec<u32> = env
            .storage()
            .instance()
            .get(&DataKey::PlayerProgress(from.clone(), set_id))
            .unwrap_or(Vec::new(&env.clone()));
        if !from_progress.contains(&achievement_id) {
            panic!("not owned");
        }
        // remove from 'from'
        let mut new_from = Vec::new(&env.clone());
        for a in from_progress.iter() {
            let v = a.clone();
            if v != achievement_id {
                new_from.push_back(v);
            }
        }
        env.storage()
            .instance()
            .set(&DataKey::PlayerProgress(from, set_id), &new_from);

        // add to 'to'
        let mut to_progress: Vec<u32> = env
            .storage()
            .instance()
            .get(&DataKey::PlayerProgress(to.clone(), set_id))
            .unwrap_or(Vec::new(&env.clone()));
        if !to_progress.contains(&achievement_id) {
            to_progress.push_back(achievement_id);
            env.storage()
                .instance()
                .set(&DataKey::PlayerProgress(to.clone(), set_id), &to_progress);
        }
    }

    // Bonus querying and withdrawal accounting (internal)
    pub fn bonus_of(env: Env, player: Address) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::Bonus(player))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_set_and_progress() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCollection);
        let client = AchievementCollectionClient::new(&env, &contract_id);

        client.initialize();

        let name = String::from_str(&env, "Starter Set");
        let mut ach = Vec::new(&env);
        ach.push_back(1);
        ach.push_back(2);
        ach.push_back(3);

        let set_id = client.create_set(&name, &ach, &Rarity::Common, &None, &100);
        assert_eq!(set_id, 1);

        let user = Address::generate(&env);
        assert_eq!(client.is_completed(&user, &set_id), false);

        // record achievements
        assert_eq!(client.record_achievement(&user, &1), false);
        assert_eq!(client.record_achievement(&user, &2), false);
        // completion on third
        assert_eq!(client.record_achievement(&user, &3), true);

        let progress = client.progress(&user, &set_id);
        assert_eq!(progress.len(), 3);
        assert_eq!(client.bonus_of(&user), 100);

        // re-recording an already owned achievement should not add bonus again
        assert_eq!(client.record_achievement(&user, &3), true);
        assert_eq!(client.bonus_of(&user), 100);
    }

    #[test]
    fn test_limited_cap_and_trading() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementCollection);
        let client = AchievementCollectionClient::new(&env, &contract_id);

        client.initialize();
        let name = String::from_str(&env, "Limited Set");
        let mut ach = Vec::new(&env);
        ach.push_back(10);
        ach.push_back(20);

        // cap 1 per achievement
        let set_id = client.create_set(&name, &ach, &Rarity::Rare, &Some(1), &50);

        let a = Address::generate(&env);
        let b = Address::generate(&env);

        // A claims 10
        client.record_achievement(&a, &10);
        // B cannot claim 10 due to cap; but can receive via transfer
        // A transfers 10 to B
        client.transfer_progress(&a, &b, &10);

        let ap = client.progress(&a, &set_id);
        assert_eq!(ap.len(), 0);
        let bp = client.progress(&b, &set_id);
        assert_eq!(bp.len(), 1);

        // B tries to claim again 10 - already has it, no change
        assert_eq!(client.record_achievement(&b, &10), false);
    }
}
