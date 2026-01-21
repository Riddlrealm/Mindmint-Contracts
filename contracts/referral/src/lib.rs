#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, token,
    Address, Env, String, Symbol, Vec,
};

//
// ──────────────────────────────────────────────────────────
// DATA KEYS
// ──────────────────────────────────────────────────────────
//

#[contracttype]
pub enum DataKey {
    /// Maps user Address to their referral code (String)
    ReferralCode(Address),
    /// Maps referral code (String) to owner Address (reverse lookup)
    CodeOwner(String),
    /// Maps referee Address to referrer Address
    Referral(Address),
    /// Maps referrer Address to count of successful referrals (u32)
    ReferralCount(Address),
    /// Maps referrer Address to list of referees (Vec<Address>)
    ReferralsList(Address),
    /// Global referral statistics
    ReferralStats,
    /// Contract configuration
    Config,
    /// Admin address
    Admin,
    /// Counter for generating unique codes
    CodeCounter,
}

//
// ──────────────────────────────────────────────────────────
// STRUCTS
// ──────────────────────────────────────────────────────────
//

#[contracttype]
#[derive(Clone, Debug)]
pub struct ReferralStats {
    pub total_referrals: u32,
    pub total_rewarded_referrals: u32,
    pub total_referrer_rewards: i128,
    pub total_referee_rewards: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Config {
    pub reward_token: Address,
    pub referrer_reward: i128,      // Reward for referrer
    pub referee_reward: i128,       // Reward for referee
    pub max_referrals_per_user: u32, // Maximum referrals allowed per user
    pub min_referral_code_length: u32, // Minimum length for referral codes
}

//
// ──────────────────────────────────────────────────────────
// CONTRACT
// ──────────────────────────────────────────────────────────
//

#[contract]
pub struct ReferralContract;

#[contractimpl]
impl ReferralContract {
    // ───────────── INITIALIZATION ─────────────

    /// Initialize the referral contract
    /// 
    /// # Arguments
    /// * `admin` - Admin address who can update configuration
    /// * `reward_token` - Address of the reward token contract
    /// * `referrer_reward` - Amount of tokens to reward referrer
    /// * `referee_reward` - Amount of tokens to reward referee
    /// * `max_referrals_per_user` - Maximum number of referrals per user
    pub fn initialize(
        env: Env,
        admin: Address,
        reward_token: Address,
        referrer_reward: i128,
        referee_reward: i128,
        max_referrals_per_user: u32,
    ) {
        admin.require_auth();

        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::CodeCounter, &0u32);

        let reward_token_clone = reward_token.clone();
        let config = Config {
            reward_token,
            referrer_reward,
            referee_reward,
            max_referrals_per_user,
            min_referral_code_length: 6,
        };
        env.storage().instance().set(&DataKey::Config, &config);

        let stats = ReferralStats {
            total_referrals: 0,
            total_rewarded_referrals: 0,
            total_referrer_rewards: 0,
            total_referee_rewards: 0,
        };
        env.storage().instance().set(&DataKey::ReferralStats, &stats);

        env.events().publish(
            (Symbol::new(&env, "init"), Symbol::new(&env, "referral")),
            (admin, reward_token_clone, referrer_reward, referee_reward),
        );
    }

    // ───────────── REFERRAL CODE GENERATION ─────────────

    /// Generate a unique referral code for a user
    /// 
    /// # Returns
    /// Generated referral code as String
    pub fn generate_referral_code(env: Env, user: Address) -> String {
        user.require_auth();

        // Check if user already has a referral code
        if env.storage().instance().has(&DataKey::ReferralCode(user.clone())) {
            panic!("Referral code already exists");
        }

        // Generate unique code using counter
        let mut counter: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CodeCounter)
            .unwrap_or(0);
        
        // Create code from counter (ensures uniqueness)
        // Use timestamp as additional entropy
        let timestamp = env.ledger().timestamp();
        let timestamp_bytes = timestamp.to_be_bytes();
        
        // Combine counter and timestamp for code generation
        let mut code_bytes = [0u8; 12];
        let counter_bytes = counter.to_be_bytes();
        
        // Mix counter and timestamp
        for i in 0..4 {
            code_bytes[i] = counter_bytes[i];
        }
        for i in 0..8.min(timestamp_bytes.len()) {
            code_bytes[i + 4] = timestamp_bytes[i];
        }

        // Convert to base32-like string (simplified for Soroban)
        let code = Self::bytes_to_code(&env, &code_bytes);
        
        counter += 1;
        env.storage().instance().set(&DataKey::CodeCounter, &counter);

        // Store bidirectional mapping
        env.storage().instance().set(&DataKey::ReferralCode(user.clone()), &code.clone());
        env.storage().instance().set(&DataKey::CodeOwner(code.clone()), &user);

        // Initialize referral count
        env.storage().instance().set(&DataKey::ReferralCount(user.clone()), &0u32);
        env.storage()
            .instance()
            .set(&DataKey::ReferralsList(user.clone()), &Vec::<Address>::new(&env));

        env.events().publish(
            (Symbol::new(&env, "referral_code_generated"), user),
            code.clone(),
        );

        code
    }

    /// Helper function to convert bytes to code string
    fn bytes_to_code(env: &Env, bytes: &[u8; 12]) -> String {
        // Simple encoding: convert bytes to alphanumeric string
        // Using alphanumeric characters only
        const CHARS: &[u8] = b"0123456789ABCDEFGHJKLMNPQRSTUVWXYZ";
        
        // Build string from bytes
        let mut code_bytes = [0u8; 12];
        for i in 0..12 {
            let idx = (bytes[i] as usize) % CHARS.len();
            code_bytes[i] = CHARS[idx];
        }
        
        // Convert bytes to string
        String::from_bytes(env, &code_bytes)
    }

    /// Get referral code for a user
    pub fn get_referral_code(env: Env, user: Address) -> Option<String> {
        env.storage().instance().get(&DataKey::ReferralCode(user))
    }

    /// Get owner of a referral code
    pub fn get_code_owner(env: Env, code: String) -> Option<Address> {
        env.storage().instance().get(&DataKey::CodeOwner(code))
    }

    // ───────────── REFERRAL REGISTRATION ─────────────

    /// Register as a referee with a referral code
    /// 
    /// # Arguments
    /// * `referee` - Address of the new user (referee)
    /// * `referral_code` - Referral code of the referrer
    /// 
    /// # Returns
    /// Returns true if registration successful and rewards distributed
    pub fn register_with_referral_code(env: Env, referee: Address, referral_code: String) -> bool {
        referee.require_auth();
        
        Self::assert_initialized(&env);

        // Anti-gaming: Prevent duplicate registration
        if env.storage().instance().has(&DataKey::Referral(referee.clone())) {
            panic!("Already registered with a referral code");
        }

        // Validate and get referrer
        let referrer: Address = match env.storage().instance().get(&DataKey::CodeOwner(referral_code.clone())) {
            Some(addr) => addr,
            None => panic!("Invalid referral code"),
        };

        // Anti-gaming: Prevent self-referral
        if referrer == referee {
            panic!("Cannot refer yourself");
        }

        // Anti-gaming: Check referral limit
        let referral_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ReferralCount(referrer.clone()))
            .unwrap_or(0);
        
        let config: Config = env.storage().instance().get(&DataKey::Config).unwrap();
        if referral_count >= config.max_referrals_per_user {
            panic!("Referrer has reached maximum referral limit");
        }

        // Store referral relationship
        env.storage()
            .instance()
            .set(&DataKey::Referral(referee.clone()), &referrer.clone());

        // Update referral count
        let new_count = referral_count + 1;
        env.storage()
            .instance()
            .set(&DataKey::ReferralCount(referrer.clone()), &new_count);

        // Add to referrer's referrals list
        let mut referrals_list: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::ReferralsList(referrer.clone()))
            .unwrap_or(Vec::new(&env));
        referrals_list.push_back(referee.clone());
        env.storage()
            .instance()
            .set(&DataKey::ReferralsList(referrer.clone()), &referrals_list);

        // Update global statistics
        let mut stats: ReferralStats = env
            .storage()
            .instance()
            .get(&DataKey::ReferralStats)
            .unwrap();
        stats.total_referrals += 1;

        // Distribute rewards
        let rewards_distributed = Self::distribute_rewards(&env, referrer.clone(), referee.clone(), &config);
        
        if rewards_distributed {
            stats.total_rewarded_referrals += 1;
            stats.total_referrer_rewards += config.referrer_reward;
            stats.total_referee_rewards += config.referee_reward;
        }

        env.storage().instance().set(&DataKey::ReferralStats, &stats);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "referral_registered"), referee),
            (referrer, referral_code, rewards_distributed),
        );

        rewards_distributed
    }

    // ───────────── REWARD DISTRIBUTION ─────────────

    /// Distribute rewards to both referrer and referee
    /// Note: This requires the contract to have authorization to mint tokens
    /// or have sufficient balance to transfer from itself
    fn distribute_rewards(env: &Env, referrer: Address, referee: Address, config: &Config) -> bool {
        // For reward_token contract, we need to check balance first
        // The contract should have been funded with tokens via deposit_reward_tokens
        // We'll use the token client to check and transfer
        
        // Try to use token::Client first (for standard token interface)
        // If that doesn't work, we'll need to interact with reward_token directly
        let total_needed = config.referrer_reward + config.referee_reward;
        
        // Check contract balance
        let contract_balance = Self::get_token_balance(env, &config.reward_token, &env.current_contract_address());
        
        if contract_balance < total_needed {
            // Insufficient balance - still record referral but don't reward
            env.events().publish(
                (Symbol::new(env, "reward_failed"), Symbol::new(env, "insufficient_balance")),
                (referrer, referee, total_needed, contract_balance),
            );
            return false;
        }

        // Transfer rewards using token client
        let token_client = token::Client::new(env, &config.reward_token);
        
        // Transfer to referrer
        if config.referrer_reward > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &referrer,
                &config.referrer_reward,
            );
        }

        // Transfer to referee
        if config.referee_reward > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &referee,
                &config.referee_reward,
            );
        }

        env.events().publish(
            (Symbol::new(env, "rewards_distributed"), referrer),
            (referee, config.referrer_reward, config.referee_reward),
        );

        true
    }

    /// Helper to get token balance (works with both standard tokens and custom reward_token)
    fn get_token_balance(env: &Env, token_addr: &Address, account: &Address) -> i128 {
        // Try token::Client first (standard interface)
        let token_client = token::Client::new(env, token_addr);
        token_client.balance(account)
    }

    /// Get referrer for a given referee
    pub fn get_referrer(env: Env, referee: Address) -> Option<Address> {
        env.storage().instance().get(&DataKey::Referral(referee))
    }

    /// Get referral count for a user
    pub fn get_referral_count(env: Env, user: Address) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ReferralCount(user))
            .unwrap_or(0)
    }

    /// Get list of referrals for a user
    pub fn get_referrals(env: Env, user: Address) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::ReferralsList(user))
            .unwrap_or(Vec::new(&env))
    }

    // ───────────── STATISTICS ─────────────

    /// Get global referral statistics
    pub fn get_statistics(env: Env) -> ReferralStats {
        env.storage()
            .instance()
            .get(&DataKey::ReferralStats)
            .unwrap()
    }

    // ───────────── ADMIN FUNCTIONS ─────────────

    /// Update configuration (admin only)
    pub fn update_config(
        env: Env,
        admin: Address,
        referrer_reward: Option<i128>,
        referee_reward: Option<i128>,
        max_referrals_per_user: Option<u32>,
    ) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut config: Config = env.storage().instance().get(&DataKey::Config).unwrap();
        
        if let Some(reward) = referrer_reward {
            config.referrer_reward = reward;
        }
        if let Some(reward) = referee_reward {
            config.referee_reward = reward;
        }
        if let Some(max) = max_referrals_per_user {
            config.max_referrals_per_user = max;
        }

        env.storage().instance().set(&DataKey::Config, &config);

        env.events().publish(
            (Symbol::new(&env, "config_updated"), admin),
            config.clone(),
        );
    }

    /// Deposit reward tokens to contract (admin only)
    /// This transfers tokens from admin to the contract for reward distribution
    pub fn deposit_reward_tokens(env: Env, admin: Address, amount: i128) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let config: Config = env.storage().instance().get(&DataKey::Config).unwrap();
        let token_client = token::Client::new(&env, &config.reward_token);

        token_client.transfer(&admin, &env.current_contract_address(), &amount);

        env.events().publish(
            (Symbol::new(&env, "tokens_deposited"), admin),
            amount,
        );
    }

    // ───────────── HELPERS ─────────────

    fn assert_initialized(env: &Env) {
        if !env.storage().instance().has(&DataKey::Admin) {
            panic!("Contract not initialized");
        }
    }

    fn assert_admin(env: &Env, user: &Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != *user {
            panic!("Admin only");
        }
    }

    /// Get current configuration
    pub fn get_config(env: Env) -> Config {
        env.storage().instance().get(&DataKey::Config).unwrap()
    }
}

#[cfg(test)]
mod test;
