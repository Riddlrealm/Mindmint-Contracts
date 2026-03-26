#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Vec, Symbol, IntoVal};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsurancePolicy {
    pub holder: Address,
    pub premium_paid: i128,
    pub coverage_percent: u32,  // In basis points (10000 = 100%)
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
    pub base_rate: i128,  // Base rate per attempt
    pub max_coverage_percent: u32,  // Max coverage percent in basis points
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
            max_coverage_percent: 8000,  // 80% max coverage
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
        
        // Validate coverage percent doesn't exceed maximum
        if coverage_percent > config.max_coverage_percent {
            panic!("Coverage percent exceeds maximum");
        }
        
        // Validate inputs
        if attempts == 0 || attempts > 100 {
            panic!("Invalid attempts count");
        }
        
        if duration == 0 || duration > 365 * 24 * 60 * 60 {  // Max 1 year
            panic!("Invalid duration");
        }
        
        // Calculate premium: attempts * base_rate * coverage_percent / 10000
        let premium = (attempts as i128) * config.base_rate * (coverage_percent as i128) / 10000i128;
        
        if premium <= 0 {
            panic!("Premium must be positive");
        }
        
        // Transfer premium from holder to contract
        let token_client = token::Client::new(&env, &config.payment_token);
        token_client.transfer(&holder, &env.current_contract_address(), &premium);
        
        // Generate policy ID
        let policy_id: u64 = env.storage().persistent().get(&DataKey::PolicyCounter).unwrap_or(0);
        let new_policy_id = policy_id + 1;
        env.storage().persistent().set(&DataKey::PolicyCounter, &new_policy_id);
        
        // Create policy
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
        
        // Store policy
        env.storage().persistent().set(&DataKey::Policy(new_policy_id), &policy);
        
        // Add to user's policies
        let mut user_policies: Vec<u64> = env.storage()
            .persistent()
            .get(&DataKey::UserPolicies(holder.clone()))
            .unwrap_or(Vec::new(&env));
        user_policies.push_back(new_policy_id);
        env.storage().persistent().set(&DataKey::UserPolicies(holder), &user_policies);
        
        // Emit event
        env.events().publish(
            (Symbol::new(&env, "policy_purchased"), new_policy_id.into_val(&env)),
            (holder.into_val(&env), attempts.into_val(&env), coverage_percent.into_val(&env), (current_time + duration).into_val(&env)),
        );
        
        new_policy_id
    }

    pub fn file_claim(env: Env, policy_id: u64, loss_amount: i128) -> i128 {
        let mut policy: InsurancePolicy = env.storage()
            .persistent()
            .get(&DataKey::Policy(policy_id))
            .expect("Policy not found");
        
        // Check if policy is active
        if !policy.active {
            panic!("Policy is not active");
        }
        
        // Check if policy has expired
        let current_time = env.ledger().timestamp();
        if current_time > policy.expires_at {
            panic!("Policy has expired");
        }
        
        // Check if attempts are available
        if policy.attempts_used >= policy.attempts_covered {
            panic!("No attempts remaining");
        }
        
        // Validate loss amount
        if loss_amount <= 0 {
            panic!("Loss amount must be positive");
        }
        
        // Calculate payout: loss_amount * coverage_percent / 10000
        let payout = loss_amount * (policy.coverage_percent as i128) / 10000i128;
        
        if payout <= 0 {
            panic!("Payout must be positive");
        }
        
        // Update policy attempts used
        policy.attempts_used += 1;
        
        // Check if policy is now exhausted
        if policy.attempts_used >= policy.attempts_covered {
            policy.active = false;
            env.events().publish(
                (Symbol::new(&env, "policy_expired"), policy_id.into_val(&env)),
                policy.holder.into_val(&env),
            );
        }
        
        // Store updated policy
        env.storage().persistent().set(&DataKey::Policy(policy_id), &policy);
        
        // Transfer payout to holder
        let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        let token_client = token::Client::new(&env, &config.payment_token);
        token_client.transfer(&env.current_contract_address(), &policy.holder, &payout);
        
        // Emit event
        env.events().publish(
            (Symbol::new(&env, "claim_paid"), policy_id.into_val(&env)),
            (policy.holder.into_val(&env), payout.into_val(&env)),
        );
        
        payout
    }

    pub fn get_policy(env: Env, policy_id: u64) -> Option<InsurancePolicy> {
        let mut policy: Option<InsurancePolicy> = env.storage().persistent().get(&DataKey::Policy(policy_id));
        
        // Check if policy should be marked as expired due to time
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

    pub fn set_base_rate(env: Env, admin: Address, new_rate: i128) {
        admin.require_auth();
        
        let mut config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        Self::assert_admin(&env, &admin);
        
        if new_rate <= 0 {
            panic!("Base rate must be positive");
        }
        
        config.base_rate = new_rate;
        env.storage().persistent().set(&DataKey::Config, &config);
    }

    pub fn set_max_coverage_percent(env: Env, admin: Address, new_max: u32) {
        admin.require_auth();
        
        let mut config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        Self::assert_admin(&env, &admin);
        
        if new_max == 0 || new_max > 10000 {
            panic!("Invalid max coverage percent");
        }
        
        config.max_coverage_percent = new_max;
        env.storage().persistent().set(&DataKey::Config, &config);
    }

    pub fn get_config(env: Env) -> InsuranceConfig {
        env.storage().persistent().get(&DataKey::Config).unwrap()
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
        
        env.events().publish(
            (Symbol::new(&env, "policy_expired"), policy_id.into_val(&env)),
            policy.holder.to_val(),
        );
    }

    fn assert_admin(env: &Env, admin: &Address) {
        let config: InsuranceConfig = env.storage().persistent().get(&DataKey::Config).unwrap();
        if config.admin != *admin {
            panic!("Not admin");
        }
    }
}

#[cfg(test)]
mod test;