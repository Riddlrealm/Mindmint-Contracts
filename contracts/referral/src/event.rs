use soroban_sdk::{contracttype, Address, Env, String, Symbol};

/// The payload varies per event kind.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferralEventData {
    /// (admin, reward_token, referrer_reward, referee_reward)
    Init(Address, Address, i128, i128),
    /// (user, code)
    ReferralCodeGenerated(Address, String),
    /// (referee, referrer, code, rewards_distributed)
    ReferralRegistered(Address, Address, String, bool),
    /// (referrer, referee, total_needed, available)
    RewardFailed(Address, Address, i128, i128),
    /// (referrer, referee, referrer_reward, referee_reward)
    RewardsDistributed(Address, Address, i128, i128),
    /// (admin)
    ConfigUpdated(Address),
    /// (admin, amount)
    TokensDeposited(Address, i128),
}

/// Emit a structured referral event.
///
/// The first topic is always the event kind symbol (e.g. "referral_registered").
/// Additional indexed fields are published as separate topics for efficient
/// filtering by off-chain indexers.
pub fn emit_referral_event(env: &Env, kind: Symbol, data: &ReferralEventData) {
    match data {
        ReferralEventData::Init(
            admin,
            reward_token,
            referrer_reward,
            referee_reward,
        ) => {
            env.events().publish(
                (kind, Symbol::new(env, "init")),
                (
                    admin.clone(),
                    reward_token.clone(),
                    *referrer_reward,
                    *referee_reward,
                ),
            );
        }
        ReferralEventData::ReferralCodeGenerated(user, code) => {
            env.events().publish(
                (kind, Symbol::new(env, "referral_code_generated")),
                (user.clone(), code.clone()),
            );
        }
        ReferralEventData::ReferralRegistered(
            referee,
            referrer,
            code,
            rewards_distributed,
        ) => {
            env.events().publish(
                (kind, Symbol::new(env, "referral_registered")),
                (
                    referee.clone(),
                    referrer.clone(),
                    code.clone(),
                    *rewards_distributed,
                ),
            );
        }
        ReferralEventData::RewardFailed(
            referrer,
            referee,
            total_needed,
            available,
        ) => {
            env.events().publish(
                (
                    kind,
                    Symbol::new(env, "reward_failed"),
                    Symbol::new(env, "insufficient_balance"),
                ),
                (
                    referrer.clone(),
                    referee.clone(),
                    *total_needed,
                    *available,
                ),
            );
        }
        ReferralEventData::RewardsDistributed(
            referrer,
            referee,
            referrer_reward,
            referee_reward,
        ) => {
            env.events().publish(
                (kind, Symbol::new(env, "rewards_distributed")),
                (
                    referrer.clone(),
                    referee.clone(),
                    *referrer_reward,
                    *referee_reward,
                ),
            );
        }
        ReferralEventData::ConfigUpdated(admin) => {
            env.events()
                .publish((kind, Symbol::new(env, "config_updated")), admin.clone());
        }
        ReferralEventData::TokensDeposited(admin, amount) => {
            env.events().publish(
                (kind, Symbol::new(env, "tokens_deposited")),
                (admin.clone(), *amount),
            );
        }
    }
}

/// Convenience helpers that construct the data and emit in one call.
pub mod emit {
    use super::*;

    pub fn init(
        env: &Env,
        admin: &Address,
        reward_token: &Address,
        referrer_reward: i128,
        referee_reward: i128,
    ) {
        emit_referral_event(
            env,
            Symbol::new(env, "init"),
            &ReferralEventData::Init(
                admin.clone(),
                reward_token.clone(),
                referrer_reward,
                referee_reward,
            ),
        );
    }

    pub fn referral_code_generated(env: &Env, user: &Address, code: &String) {
        emit_referral_event(
            env,
            Symbol::new(env, "referral_code_generated"),
            &ReferralEventData::ReferralCodeGenerated(user.clone(), code.clone()),
        );
    }

    pub fn referral_registered(
        env: &Env,
        referee: &Address,
        referrer: &Address,
        code: &String,
        rewards_distributed: bool,
    ) {
        emit_referral_event(
            env,
            Symbol::new(env, "referral_registered"),
            &ReferralEventData::ReferralRegistered(
                referee.clone(),
                referrer.clone(),
                code.clone(),
                rewards_distributed,
            ),
        );
    }

    pub fn reward_failed(
        env: &Env,
        referrer: &Address,
        referee: &Address,
        total_needed: i128,
        available: i128,
    ) {
        emit_referral_event(
            env,
            Symbol::new(env, "reward_failed"),
            &ReferralEventData::RewardFailed(
                referrer.clone(),
                referee.clone(),
                total_needed,
                available,
            ),
        );
    }

    pub fn rewards_distributed(
        env: &Env,
        referrer: &Address,
        referee: &Address,
        referrer_reward: i128,
        referee_reward: i128,
    ) {
        emit_referral_event(
            env,
            Symbol::new(env, "rewards_distributed"),
            &ReferralEventData::RewardsDistributed(
                referrer.clone(),
                referee.clone(),
                referrer_reward,
                referee_reward,
            ),
        );
    }

    pub fn config_updated(env: &Env, admin: &Address) {
        emit_referral_event(
            env,
            Symbol::new(env, "config_updated"),
            &ReferralEventData::ConfigUpdated(admin.clone()),
        );
    }

    pub fn tokens_deposited(env: &Env, admin: &Address, amount: i128) {
        emit_referral_event(
            env,
            Symbol::new(env, "tokens_deposited"),
            &ReferralEventData::TokensDeposited(admin.clone(), amount),
        );
    }
}
