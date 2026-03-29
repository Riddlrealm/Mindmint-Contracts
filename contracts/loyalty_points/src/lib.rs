#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Vec,
};

// ──────────────────────────────────────────────────────────
// CONSTANTS
// ──────────────────────────────────────────────────────────

/// 12 months in seconds (365 days).
const EXPIRY_WINDOW: u64 = 365 * 24 * 60 * 60;

// ──────────────────────────────────────────────────────────
// ERROR CODES
// ──────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum LoyaltyError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InsufficientBalance = 4,
    OptionNotFound = 5,
    OptionDisabled = 6,
    PointsExpired = 7,
    NonTransferable = 8,
}

// ──────────────────────────────────────────────────────────
// DATA KEYS
// ──────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Oracle,
    PointsBalance(Address),
    RedemptionOption(u32),
    NextOptionId,
    AllPlayers,
}

// ──────────────────────────────────────────────────────────
// STRUCTS
// ──────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct PointsBalance {
    pub player: Address,
    pub total_earned: u64,
    pub total_redeemed: u64,
    pub current_balance: u64,
    pub last_activity: u64,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RewardType {
    Token = 0,
    Discount = 1,
    Item = 2,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct RedemptionOption {
    pub id: u32,
    pub name: String,
    pub points_cost: u64,
    pub reward_type: RewardType,
    pub reward_value: u64,
    pub enabled: bool,
}

// ──────────────────────────────────────────────────────────
// CONTRACT
// ──────────────────────────────────────────────────────────

#[contract]
pub struct LoyaltyPointsContract;

#[contractimpl]
impl LoyaltyPointsContract {
    /// Initialize the contract with an admin and an oracle address.
    pub fn initialize(
        env: Env,
        admin: Address,
        oracle: Address,
    ) -> Result<(), LoyaltyError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(LoyaltyError::AlreadyInitialized);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Oracle, &oracle);
        env.storage().instance().set(&DataKey::NextOptionId, &0u32);

        let empty_players: Vec<Address> = Vec::new(&env);
        env.storage().instance().set(&DataKey::AllPlayers, &empty_players);

        Ok(())
    }

    // ───────────── ORACLE FUNCTIONS ─────────────

    /// Award points to a player. Only callable by the authorized oracle.
    pub fn award_points(
        env: Env,
        player: Address,
        amount: u64,
        reason: String,
    ) -> Result<u64, LoyaltyError> {
        let oracle: Address = env
            .storage()
            .instance()
            .get(&DataKey::Oracle)
            .ok_or(LoyaltyError::NotInitialized)?;
        oracle.require_auth();

        let now = env.ledger().timestamp();
        let key = DataKey::PointsBalance(player.clone());

        let mut balance = env
            .storage()
            .persistent()
            .get::<DataKey, PointsBalance>(&key)
            .unwrap_or(PointsBalance {
                player: player.clone(),
                total_earned: 0,
                total_redeemed: 0,
                current_balance: 0,
                last_activity: now,
            });

        // Register new player
        if balance.total_earned == 0 && balance.total_redeemed == 0 {
            let mut players: Vec<Address> = env
                .storage()
                .instance()
                .get(&DataKey::AllPlayers)
                .unwrap_or(Vec::new(&env));
            players.push_back(player.clone());
            env.storage().instance().set(&DataKey::AllPlayers, &players);
        }

        balance.current_balance += amount;
        balance.total_earned += amount;
        balance.last_activity = now;

        env.storage().persistent().set(&key, &balance);

        env.events().publish(
            (symbol_short!("awarded"), player.clone()),
            (amount, reason),
        );

        Ok(balance.current_balance)
    }

    // ───────────── PLAYER FUNCTIONS ─────────────

    /// Redeem points for a reward option. Caller must be the player.
    pub fn redeem(
        env: Env,
        player: Address,
        option_id: u32,
    ) -> Result<u64, LoyaltyError> {
        player.require_auth();

        let option = Self::get_option_or_err(&env, option_id)?;
        if !option.enabled {
            return Err(LoyaltyError::OptionDisabled);
        }

        let now = env.ledger().timestamp();
        let key = DataKey::PointsBalance(player.clone());

        let mut balance = env
            .storage()
            .persistent()
            .get::<DataKey, PointsBalance>(&key)
            .ok_or(LoyaltyError::InsufficientBalance)?;

        // Check expiry first
        if now.saturating_sub(balance.last_activity) > EXPIRY_WINDOW {
            return Err(LoyaltyError::PointsExpired);
        }

        if balance.current_balance < option.points_cost {
            return Err(LoyaltyError::InsufficientBalance);
        }

        balance.current_balance -= option.points_cost;
        balance.total_redeemed += option.points_cost;
        balance.last_activity = now;

        env.storage().persistent().set(&key, &balance);

        env.events().publish(
            (symbol_short!("redeemed"), player.clone()),
            (option_id, option.points_cost),
        );

        Ok(balance.current_balance)
    }

    // ───────────── EXPIRY ─────────────

    /// Expire a player's points if inactive for more than 12 months.
    /// Callable by anyone.
    pub fn expire_stale_points(
        env: Env,
        player: Address,
    ) -> Result<u64, LoyaltyError> {
        let now = env.ledger().timestamp();
        let key = DataKey::PointsBalance(player.clone());

        let mut balance = env
            .storage()
            .persistent()
            .get::<DataKey, PointsBalance>(&key)
            .ok_or(LoyaltyError::InsufficientBalance)?;

        if now.saturating_sub(balance.last_activity) <= EXPIRY_WINDOW {
            // Not expired — no-op
            return Ok(balance.current_balance);
        }

        let expired_amount = balance.current_balance;
        balance.current_balance = 0;

        env.storage().persistent().set(&key, &balance);

        env.events().publish(
            (symbol_short!("expired"), player.clone()),
            expired_amount,
        );

        Ok(0)
    }

    // ───────────── ADMIN FUNCTIONS ─────────────

    /// Create a new redemption option. Admin only.
    pub fn create_option(
        env: Env,
        name: String,
        points_cost: u64,
        reward_type: RewardType,
        reward_value: u64,
    ) -> Result<u32, LoyaltyError> {
        Self::require_admin(&env)?;

        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextOptionId)
            .unwrap_or(0);

        let option = RedemptionOption {
            id,
            name,
            points_cost,
            reward_type,
            reward_value,
            enabled: true,
        };

        env.storage()
            .instance()
            .set(&DataKey::RedemptionOption(id), &option);
        env.storage()
            .instance()
            .set(&DataKey::NextOptionId, &(id + 1));

        Ok(id)
    }

    /// Update an existing redemption option. Admin only.
    pub fn update_option(
        env: Env,
        option_id: u32,
        name: String,
        points_cost: u64,
        reward_type: RewardType,
        reward_value: u64,
        enabled: bool,
    ) -> Result<(), LoyaltyError> {
        Self::require_admin(&env)?;

        // Verify exists
        Self::get_option_or_err(&env, option_id)?;

        let option = RedemptionOption {
            id: option_id,
            name,
            points_cost,
            reward_type,
            reward_value,
            enabled,
        };

        env.storage()
            .instance()
            .set(&DataKey::RedemptionOption(option_id), &option);

        Ok(())
    }

    /// Disable a redemption option. Admin only.
    pub fn disable_option(env: Env, option_id: u32) -> Result<(), LoyaltyError> {
        Self::require_admin(&env)?;

        let mut option = Self::get_option_or_err(&env, option_id)?;
        option.enabled = false;

        env.storage()
            .instance()
            .set(&DataKey::RedemptionOption(option_id), &option);

        Ok(())
    }

    /// Update the oracle address. Admin only.
    pub fn set_oracle(env: Env, new_oracle: Address) -> Result<(), LoyaltyError> {
        Self::require_admin(&env)?;
        env.storage().instance().set(&DataKey::Oracle, &new_oracle);
        Ok(())
    }

    // ───────────── VIEW FUNCTIONS ─────────────

    /// Get a player's balance info.
    pub fn get_balance(env: Env, player: Address) -> PointsBalance {
        let key = DataKey::PointsBalance(player.clone());
        env.storage()
            .persistent()
            .get::<DataKey, PointsBalance>(&key)
            .unwrap_or(PointsBalance {
                player,
                total_earned: 0,
                total_redeemed: 0,
                current_balance: 0,
                last_activity: 0,
            })
    }

    /// Get a redemption option by id.
    pub fn get_option(env: Env, option_id: u32) -> Result<RedemptionOption, LoyaltyError> {
        Self::get_option_or_err(&env, option_id)
    }

    // ───────────── INTERNAL HELPERS ─────────────

    fn require_admin(env: &Env) -> Result<(), LoyaltyError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LoyaltyError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }

    fn get_option_or_err(env: &Env, option_id: u32) -> Result<RedemptionOption, LoyaltyError> {
        env.storage()
            .instance()
            .get::<DataKey, RedemptionOption>(&DataKey::RedemptionOption(option_id))
            .ok_or(LoyaltyError::OptionNotFound)
    }
}

#[cfg(test)]
mod test;
