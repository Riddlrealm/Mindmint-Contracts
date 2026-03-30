#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Vec, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsurancePolicy {
    pub holder: Address,
    pub premium_paid: i128,
    pub coverage_percent: u32,
    pub attempts_covered: u32,
    pub attempts_used: u32,
    pub expires_at: u64,
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsuranceConfig {
    pub admin: Address,
    pub payment_token: Address,
    pub base_rate: i128,
    pub max_coverage_percent: u32,
}

#[contracttype]
pub enum DataKey {
    Config,
    Policy(u64),
    PolicyCounter,
    UserPolicies(Address),
}

#[contract]
pub struct PuzzleInsuranceContract;

#[contractimpl]
impl PuzzleInsuranceContract {
    pub fn initialize(env: Env, admin: Address, payment_token: Address, base_rate: i128) {
        admin.require_auth();

        if env.storage().persistent().has(&DataKey::Config) {
            panic!("Already initialized");
        }

        let config = InsuranceConfig {
            admin: admin.clone(),
            payment_token,
            base_rate,
            max_coverage_percent: 8000,
        };

        env.storage().persistent().set(&DataKey::Config, &config);
        env.storage().persistent().set(&DataKey::PolicyCounter, &0u64);
    }

    pub fn purchase_policy(
        env: Env,
        holder: Address,
        attempts: u32,
        duration: u64,
        coverage_percent: u32,
    ) -> u64 {
        holder.require_auth();

        let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();

        if coverage_percent > config.max_coverage_percent {
            panic!("Coverage percent exceeds maximum");
        }

        if attempts == 0 || attempts > 100 {
            panic!("Invalid attempts count");
        }

        if duration == 0 || duration > 365 * 24 * 60 * 60 {
            panic!("Invalid duration");
        }

        let premium = (attempts as i128) * config.base_rate * (coverage_percent as i128) / 10000;

        if premium <= 0 {
            panic!("Premium must be positive");
        }

        let token_client = token::Client::new(&env, &config.payment_token);
        token_client.transfer(&holder, &env.current_contract_address(), &premium);

        let policy_id: u64 = env.storage().persistent().get(&DataKey::PolicyCounter).unwrap_or(0);
        let new_policy_id = policy_id + 1;
        env.storage().persistent().set(&DataKey::PolicyCounter, &new_policy_id);

        let current_time = env.ledger().timestamp();

        let policy = InsurancePolicy {
            holder: holder.clone(),
            premium_paid: premium,
            coverage_percent,
            attempts_covered: attempts,
            attempts_used: 0,
            expires_at: current_time + duration,
            active: true,
        };

        env.storage().persistent().set(&DataKey::Policy(new_policy_id), &policy);

        let mut user_policies: Vec<u64> = env.storage()
            .persistent()
            .get(&DataKey::UserPolicies(holder.clone()))
            .unwrap_or(Vec::new(&env));

        user_policies.push_back(new_policy_id);
        env.storage().persistent().set(&DataKey::UserPolicies(holder.clone()), &user_policies);

        // ✅ FIXED EVENT
        env.events().publish(
            (Symbol::new(&env, "policy_purchased"), new_policy_id),
            (holder, attempts, coverage_percent, current_time + duration),
        );

        new_policy_id
    }

    pub fn file_claim(env: Env, policy_id: u64, loss_amount: i128) -> i128 {
        let mut policy: InsurancePolicy = env.storage()
            .persistent()
            .get(&DataKey::Policy(policy_id))
            .expect("Policy not found");

        if !policy.active {
            panic!("Policy is not active");
        }

        let current_time = env.ledger().timestamp();
        if current_time > policy.expires_at {
            panic!("Policy has expired");
        }

        if policy.attempts_used >= policy.attempts_covered {
            panic!("No attempts remaining");
        }

        if loss_amount <= 0 {
            panic!("Loss amount must be positive");
        }

        let payout = loss_amount * (policy.coverage_percent as i128) / 10000;

        if payout <= 0 {
            panic!("Payout must be positive");
        }

        policy.attempts_used += 1;

        if policy.attempts_used >= policy.attempts_covered {
            policy.active = false;

            // ✅ FIXED EVENT
            env.events().publish(
                (Symbol::new(&env, "policy_expired"), policy_id),
                policy.holder.clone(),
            );
        }

        env.storage().persistent().set(&DataKey::Policy(policy_id), &policy);

        let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let token_client = token::Client::new(&env, &config.payment_token);

        token_client.transfer(&env.current_contract_address(), &policy.holder, &payout);

        // ✅ FIXED EVENT
        env.events().publish(
            (Symbol::new(&env, "claim_paid"), policy_id),
            (policy.holder, payout),
        );

        payout
    }

    pub fn get_policy(env: Env, policy_id: u64) -> Option<InsurancePolicy> {
        let mut policy: Option<InsurancePolicy> =
            env.storage().persistent().get(&DataKey::Policy(policy_id));

        if let Some(ref mut p) = policy {
            if p.active && env.ledger().timestamp() > p.expires_at {
                p.active = false;
                env.storage().persistent().set(&DataKey::Policy(policy_id), p);
            }
        }

        policy
    }

    pub fn get_user_policies(env: Env, holder: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::UserPolicies(holder))
            .unwrap_or(Vec::new(&env))
    }

    pub fn expire_policy(env: Env, policy_id: u64) {
        let mut policy: InsurancePolicy = env.storage()
            .persistent()
            .get(&DataKey::Policy(policy_id))
            .expect("Policy not found");

        if !policy.active {
            panic!("Policy already inactive");
        }

        policy.active = false;
        env.storage().persistent().set(&DataKey::Policy(policy_id), &policy);

        // ✅ FIXED EVENT
        env.events().publish(
            (Symbol::new(&env, "policy_expired"), policy_id),
            policy.holder,
        );
    }
}

#[cfg(test)]
mod test;