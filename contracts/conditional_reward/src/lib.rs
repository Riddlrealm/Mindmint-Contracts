#![no_std]

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, IntoVal, Map, Symbol,
    TryFromVal, Val, Vec,
};

#[cfg(not(test))]
const DAY_SECONDS: u64 = 86_400;
#[cfg(test)]
const DAY_SECONDS: u64 = 1;

const KEY_CONTRACT: Symbol = symbol_short!("contract");
const KEY_TOKEN_ID: Symbol = symbol_short!("token_id");
const KEY_MIN: Symbol = symbol_short!("min");
const KEY_DAYS: Symbol = symbol_short!("days");
const KEY_TIER: Symbol = symbol_short!("tier");
const KEY_ACT_TYPE: Symbol = symbol_short!("activity");

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionType {
    NftHeld,
    SolveCountGte,
    RegistrationAgeGte,
    TierGte,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Condition {
    pub condition_type: ConditionType,
    pub params: Map<Symbol, Val>,
    pub or_group: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FailedCondition {
    pub index: u32,
    pub condition_type: ConditionType,
    pub or_group: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneReward {
    pub id: u64,
    pub reward_amount: i128,
    pub conditions: Vec<Condition>,
    pub claimants: Vec<Address>,
    pub max_claims: u32,
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewardView {
    pub id: u64,
    pub reward_amount: i128,
    pub conditions: Vec<Condition>,
    pub claims_remaining: u32,
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Token,
    NextRewardId,
    Reward(u64),
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum SubscriptionTier {
    Free = 0,
    Pro = 1,
    Elite = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SocialLinksView {
    pub twitter: Option<soroban_sdk::String>,
    pub discord: Option<soroban_sdk::String>,
    pub github: Option<soroban_sdk::String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerIdentityView {
    pub address: Address,
    pub username: soroban_sdk::String,
    pub avatar_hash: Option<soroban_sdk::String>,
    pub bio_hash: Option<soroban_sdk::String>,
    pub social_links: SocialLinksView,
    pub registered_at: u64,
    pub verified: bool,
}

#[contract]
pub struct ConditionalRewardContract;

#[contractimpl]
impl ConditionalRewardContract {
    pub fn initialize(env: Env, admin: Address, token: Address) {
        admin.require_auth();

        if env.storage().persistent().has(&DataKey::Admin) {
            panic!("already initialized");
        }

        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::Token, &token);
        env.storage()
            .persistent()
            .set(&DataKey::NextRewardId, &1u64);
    }

    pub fn create_reward(
        env: Env,
        admin: Address,
        conditions: Vec<Condition>,
        reward_amount: i128,
        max_claims: u32,
    ) -> u64 {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if reward_amount <= 0 {
            panic!("reward amount must be positive");
        }
        if max_claims == 0 {
            panic!("max claims must be positive");
        }
        if conditions.is_empty() {
            panic!("conditions required");
        }

        for condition in conditions.iter() {
            Self::validate_condition(&env, &condition);
        }

        let total_funding = reward_amount
            .checked_mul(max_claims as i128)
            .unwrap_or_else(|| panic!("funding overflow"));
        let token = Self::token(&env);
        token::Client::new(&env, &token).transfer(
            &admin,
            &env.current_contract_address(),
            &total_funding,
        );

        let reward_id = Self::next_reward_id(&env);
        let reward = MilestoneReward {
            id: reward_id,
            reward_amount,
            conditions,
            claimants: Vec::new(&env),
            max_claims,
            active: true,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Reward(reward_id), &reward);
        reward_id
    }

    pub fn check_eligibility(
        env: Env,
        reward_id: u64,
        player: Address,
    ) -> (bool, Vec<FailedCondition>) {
        let reward = Self::get_reward_internal(&env, reward_id);
        Self::evaluate_reward(&env, &reward, &player)
    }

    pub fn claim(env: Env, reward_id: u64, player: Address) {
        player.require_auth();

        let mut reward = Self::get_reward_internal(&env, reward_id);
        if !reward.active {
            panic!("reward inactive");
        }
        if reward.claimants.contains(&player) {
            panic!("player already claimed");
        }
        if reward.claimants.len() >= reward.max_claims {
            panic!("max claims reached");
        }

        let (eligible, failed) = Self::evaluate_reward(&env, &reward, &player);
        if !eligible {
            if !failed.is_empty() {
                panic!("player not eligible");
            }
            panic!("eligibility check failed");
        }

        token::Client::new(&env, &Self::token(&env)).transfer(
            &env.current_contract_address(),
            &player,
            &reward.reward_amount,
        );

        reward.claimants.push_back(player.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Reward(reward_id), &reward);

        env.events().publish(
            (Symbol::new(&env, "RewardClaimed"), reward_id, player),
            reward.reward_amount,
        );
    }

    pub fn deactivate_reward(env: Env, admin: Address, reward_id: u64) -> i128 {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut reward = Self::get_reward_internal(&env, reward_id);
        if !reward.active {
            panic!("reward already inactive");
        }

        reward.active = false;
        let claims_remaining = reward.max_claims.saturating_sub(reward.claimants.len());
        let refund_amount = reward
            .reward_amount
            .checked_mul(claims_remaining as i128)
            .unwrap_or_else(|| panic!("refund overflow"));

        if refund_amount > 0 {
            token::Client::new(&env, &Self::token(&env)).transfer(
                &env.current_contract_address(),
                &admin,
                &refund_amount,
            );
        }

        env.storage()
            .persistent()
            .set(&DataKey::Reward(reward_id), &reward);
        env.events()
            .publish((Symbol::new(&env, "RewardDeactivated"), reward_id), admin);

        refund_amount
    }

    pub fn get_reward(env: Env, reward_id: u64) -> RewardView {
        let reward = Self::get_reward_internal(&env, reward_id);
        RewardView {
            id: reward.id,
            reward_amount: reward.reward_amount,
            conditions: reward.conditions,
            claims_remaining: reward.max_claims.saturating_sub(reward.claimants.len()),
            active: reward.active,
        }
    }

    fn evaluate_reward(
        env: &Env,
        reward: &MilestoneReward,
        player: &Address,
    ) -> (bool, Vec<FailedCondition>) {
        let mut results = Vec::new(env);
        for condition in reward.conditions.iter() {
            results.push_back(Self::evaluate_condition(env, &condition, player));
        }

        let mut failed = Vec::new(env);
        let mut eligible = true;
        let mut index: u32 = 0;
        while index < reward.conditions.len() {
            let condition = reward.conditions.get(index).unwrap();
            let passed = results.get(index).unwrap();

            if condition.or_group == 0 {
                if !passed {
                    eligible = false;
                    failed.push_back(FailedCondition {
                        index,
                        condition_type: condition.condition_type,
                        or_group: 0,
                    });
                }
            } else if !Self::group_satisfied(reward, &results, condition.or_group) {
                eligible = false;
                failed.push_back(FailedCondition {
                    index,
                    condition_type: condition.condition_type,
                    or_group: condition.or_group,
                });
            }

            index += 1;
        }

        (eligible, failed)
    }

    fn group_satisfied(reward: &MilestoneReward, results: &Vec<bool>, group_id: u32) -> bool {
        let mut index: u32 = 0;
        while index < reward.conditions.len() {
            let condition = reward.conditions.get(index).unwrap();
            if condition.or_group == group_id && results.get(index).unwrap() {
                return true;
            }
            index += 1;
        }
        false
    }

    fn evaluate_condition(env: &Env, condition: &Condition, player: &Address) -> bool {
        match condition.condition_type {
            ConditionType::NftHeld => {
                let nft_contract = Self::get_param::<Address>(env, &condition.params, KEY_CONTRACT)
                    .unwrap_or_else(|| panic!("missing nft contract"));
                let token_id = Self::get_param::<u32>(env, &condition.params, KEY_TOKEN_ID)
                    .unwrap_or_else(|| panic!("missing token id"));
                let result = env.try_invoke_contract::<Address, soroban_sdk::Error>(
                    &nft_contract,
                    &symbol_short!("owner_of"),
                    soroban_sdk::vec![env, token_id.into_val(env)],
                );

                matches!(result, Ok(Ok(owner)) if owner == *player)
            }
            ConditionType::SolveCountGte => {
                let proof_contract =
                    Self::get_param::<Address>(env, &condition.params, KEY_CONTRACT)
                        .unwrap_or_else(|| panic!("missing proof contract"));
                let minimum = Self::get_param::<u32>(env, &condition.params, KEY_MIN)
                    .unwrap_or_else(|| panic!("missing solve threshold"));
                let activity_type =
                    Self::get_param::<u32>(env, &condition.params, KEY_ACT_TYPE).unwrap_or(0);

                let result = env.try_invoke_contract::<u32, soroban_sdk::Error>(
                    &proof_contract,
                    &Symbol::new(env, "get_activity_count"),
                    soroban_sdk::vec![
                        env,
                        player.clone().into_val(env),
                        activity_type.into_val(env),
                    ],
                );

                matches!(result, Ok(Ok(count)) if count >= minimum)
            }
            ConditionType::RegistrationAgeGte => {
                let identity_contract =
                    Self::get_param::<Address>(env, &condition.params, KEY_CONTRACT)
                        .unwrap_or_else(|| panic!("missing identity contract"));
                let minimum_days = Self::get_param::<u64>(env, &condition.params, KEY_DAYS)
                    .unwrap_or_else(|| panic!("missing age threshold"));

                let result = env
                    .try_invoke_contract::<Option<PlayerIdentityView>, soroban_sdk::Error>(
                        &identity_contract,
                        &Symbol::new(env, "resolve_address"),
                        soroban_sdk::vec![env, player.clone().into_val(env)],
                    );

                matches!(result, Ok(Ok(Some(identity))) if env.ledger().timestamp().saturating_sub(identity.registered_at) >= minimum_days.saturating_mul(DAY_SECONDS))
            }
            ConditionType::TierGte => {
                let subscription_contract =
                    Self::get_param::<Address>(env, &condition.params, KEY_CONTRACT)
                        .unwrap_or_else(|| panic!("missing subscription contract"));
                let tier_value = Self::get_param::<u32>(env, &condition.params, KEY_TIER)
                    .unwrap_or_else(|| panic!("missing tier"));
                let required_tier = SubscriptionTier::from_u32(tier_value)
                    .unwrap_or_else(|| panic!("invalid tier"));

                let result = env.try_invoke_contract::<bool, soroban_sdk::Error>(
                    &subscription_contract,
                    &Symbol::new(env, "has_access"),
                    soroban_sdk::vec![
                        env,
                        player.clone().into_val(env),
                        required_tier.into_val(env),
                    ],
                );

                matches!(result, Ok(Ok(has_access)) if has_access)
            }
        }
    }

    fn validate_condition(env: &Env, condition: &Condition) {
        if condition.or_group == u32::MAX {
            panic!("invalid or group");
        }

        match condition.condition_type {
            ConditionType::NftHeld => {
                Self::require_param::<Address>(
                    env,
                    &condition.params,
                    KEY_CONTRACT,
                    "missing nft contract",
                );
                Self::require_param::<u32>(
                    env,
                    &condition.params,
                    KEY_TOKEN_ID,
                    "missing token id",
                );
            }
            ConditionType::SolveCountGte => {
                Self::require_param::<Address>(
                    env,
                    &condition.params,
                    KEY_CONTRACT,
                    "missing proof contract",
                );
                Self::require_param::<u32>(
                    env,
                    &condition.params,
                    KEY_MIN,
                    "missing solve threshold",
                );
                if Self::get_param::<u32>(env, &condition.params, KEY_ACT_TYPE).is_none() {
                    let _ = 0u32;
                }
            }
            ConditionType::RegistrationAgeGte => {
                Self::require_param::<Address>(
                    env,
                    &condition.params,
                    KEY_CONTRACT,
                    "missing identity contract",
                );
                Self::require_param::<u64>(
                    env,
                    &condition.params,
                    KEY_DAYS,
                    "missing age threshold",
                );
            }
            ConditionType::TierGte => {
                Self::require_param::<Address>(
                    env,
                    &condition.params,
                    KEY_CONTRACT,
                    "missing subscription contract",
                );
                let tier_value =
                    Self::require_param::<u32>(env, &condition.params, KEY_TIER, "missing tier");
                if SubscriptionTier::from_u32(tier_value).is_none() {
                    panic!("invalid tier");
                }
            }
        }
    }

    fn require_param<T: TryFromVal<Env, Val>>(
        env: &Env,
        params: &Map<Symbol, Val>,
        key: Symbol,
        error: &str,
    ) -> T {
        Self::get_param(env, params, key).unwrap_or_else(|| panic!("{}", error))
    }

    fn get_param<T: TryFromVal<Env, Val>>(
        env: &Env,
        params: &Map<Symbol, Val>,
        key: Symbol,
    ) -> Option<T> {
        let value = params.get(key)?;
        T::try_from_val(env, &value).ok()
    }

    fn get_reward_internal(env: &Env, reward_id: u64) -> MilestoneReward {
        env.storage()
            .persistent()
            .get(&DataKey::Reward(reward_id))
            .unwrap_or_else(|| panic!("reward not found"))
    }

    fn next_reward_id(env: &Env) -> u64 {
        let reward_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::NextRewardId)
            .unwrap_or(1);
        env.storage()
            .persistent()
            .set(&DataKey::NextRewardId, &(reward_id + 1));
        reward_id
    }

    fn assert_admin(env: &Env, admin: &Address) {
        let stored_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("not initialized"));
        if stored_admin != *admin {
            panic!("admin only");
        }
    }

    fn token(env: &Env) -> Address {
        env.storage()
            .persistent()
            .get(&DataKey::Token)
            .unwrap_or_else(|| panic!("not initialized"))
    }
}

impl SubscriptionTier {
    fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Free),
            1 => Some(Self::Pro),
            2 => Some(Self::Elite),
            _ => None,
        }
    }
}
