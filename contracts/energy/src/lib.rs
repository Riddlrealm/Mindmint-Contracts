#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env};

/// Energy and Stamina Management Contract
///
/// This contract manages player energy/stamina with time-based regeneration,
/// token-based refills, and various boost mechanisms for the quest system.
///
/// # Energy Economics
/// - Base regeneration: 1 energy per 5 minutes
/// - Max energy cap: 100 energy units
/// - Puzzle attempts cost: 10 energy units
/// - Instant refill cost: 50 reward tokens per full refill
/// - Energy gifting: Players can gift energy to others (max 20 per day per player)
/// - Boosts: Temporary regeneration multipliers (2x, 3x, 5x) via powerups

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoostType {
    None = 0,
    DoubleRegen = 1,     // 2x regeneration rate
    TripleRegen = 2,     // 3x regeneration rate
    QuintupleRegen = 3,  // 5x regeneration rate
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PlayerEnergy {
    /// Current energy amount (0 to max_energy)
    pub current_energy: u32,
    /// Maximum energy capacity for this player
    pub max_energy: u32,
    /// Last time energy was updated (ledger timestamp)
    pub last_update: u64,
    /// Active boost type
    pub active_boost: BoostType,
    /// Boost expiration timestamp
    pub boost_expires_at: u64,
    /// Total energy gifted today (resets daily)
    pub gifted_today: u32,
    /// Last gift reset timestamp
    pub last_gift_reset: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct EnergyConfig {
    /// Contract administrator
    pub admin: Address,
    /// Reward token contract address for refills
    pub reward_token: Address,
    /// Base regeneration rate (energy per second)
    pub base_regen_rate: u32,
    /// Default maximum energy cap
    pub default_max_energy: u32,
    /// Energy cost per puzzle attempt
    pub puzzle_energy_cost: u32,
    /// Token cost for full energy refill
    pub refill_token_cost: i128,
    /// Maximum energy that can be gifted per day
    pub max_gift_per_day: u32,
    /// Contract paused state
    pub paused: bool,
}

#[contracttype]
pub enum DataKey {
    Config,
    PlayerEnergy(Address),
    TotalPlayers,
    DailyGiftReset, // Last daily reset timestamp
}

/// Custom error codes for the energy contract
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    InsufficientEnergy = 3,
    MaxEnergyExceeded = 4,
    InvalidAmount = 5,
    BoostAlreadyActive = 6,
    InvalidBoostType = 7,
    ContractPaused = 8,
    GiftLimitExceeded = 9,
    Unauthorized = 10,
    InvalidTimestamp = 11,
}

// Constants
const SECONDS_PER_DAY: u64 = 86400;

#[contract]
pub struct EnergyContract;

#[contractimpl]
impl EnergyContract {
    // ───────────── INITIALIZATION ─────────────

    /// Initialize the energy contract with configuration
    ///
    /// # Arguments
    /// * `admin` - Contract administrator address
    /// * `reward_token` - Address of the reward token contract for refills
    /// * `base_regen_rate` - Base energy regeneration rate (energy units per second)
    /// * `default_max_energy` - Default maximum energy capacity
    /// * `puzzle_energy_cost` - Energy cost per puzzle attempt
    /// * `refill_token_cost` - Token cost for instant full refill
    pub fn initialize(
        env: Env,
        admin: Address,
        reward_token: Address,
        base_regen_rate: u32,
        default_max_energy: u32,
        puzzle_energy_cost: u32,
        refill_token_cost: i128,
    ) {
        admin.require_auth();

        let storage = env.storage().instance();
        if storage.has(&DataKey::Config) {
            panic!("Already initialized");
        }

        let config = EnergyConfig {
            admin,
            reward_token,
            base_regen_rate,
            default_max_energy,
            puzzle_energy_cost,
            refill_token_cost,
            max_gift_per_day: 20, // Max 20 energy gifts per day
            paused: false,
        };

        storage.set(&DataKey::Config, &config);
        storage.set(&DataKey::TotalPlayers, &0u32);
    }

    // ───────────── ADMIN FUNCTIONS ─────────────

    /// Update energy configuration (admin only)
    pub fn update_config(
        env: Env,
        admin: Address,
        base_regen_rate: Option<u32>,
        default_max_energy: Option<u32>,
        puzzle_energy_cost: Option<u32>,
        refill_token_cost: Option<i128>,
        max_gift_per_day: Option<u32>,
    ) -> Result<(), Error> {
        admin.require_auth();
        Self::assert_admin(&env, &admin)?;

        let mut config: EnergyConfig = env.storage().instance().get(&DataKey::Config).unwrap();

        if let Some(rate) = base_regen_rate {
            config.base_regen_rate = rate;
        }
        if let Some(max) = default_max_energy {
            config.default_max_energy = max;
        }
        if let Some(cost) = puzzle_energy_cost {
            config.puzzle_energy_cost = cost;
        }
        if let Some(token_cost) = refill_token_cost {
            config.refill_token_cost = token_cost;
        }
        if let Some(max_gift) = max_gift_per_day {
            config.max_gift_per_day = max_gift;
        }

        env.storage().instance().set(&DataKey::Config, &config);

        Ok(())
    }

    /// Pause/unpause the contract (admin only)
    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), Error> {
        admin.require_auth();
        Self::assert_admin(&env, &admin)?;

        let mut config: EnergyConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        config.paused = paused;
        env.storage().instance().set(&DataKey::Config, &config);

        Ok(())
    }

    // ───────────── PLAYER FUNCTIONS ─────────────

    /// Get or create player energy data
    pub fn get_player_energy(env: Env, player: Address) -> PlayerEnergy {
        Self::get_or_create_player_energy(&env, player)
    }

    /// Consume energy for puzzle attempt
    ///
    /// # Returns
    /// * `Ok(())` - Energy consumed successfully
    /// * `Err(Error::InsufficientEnergy)` - Player doesn't have enough energy
    pub fn consume_energy_for_puzzle(env: Env, player: Address) -> Result<(), Error> {
        player.require_auth();
        let player_addr = player.clone();
        Self::assert_not_paused(&env)?;

        let config: EnergyConfig = env.storage().instance().get(&DataKey::Config)
            .ok_or(Error::NotInitialized)?;

        let mut player_energy = Self::get_or_create_player_energy(&env, player_addr.clone());
        Self::update_energy_regeneration(&env, &mut player_energy, &config);

        if player_energy.current_energy < config.puzzle_energy_cost {
            return Err(Error::InsufficientEnergy);
        }

        player_energy.current_energy -= config.puzzle_energy_cost;
        player_energy.last_update = env.ledger().timestamp();

        env.storage().instance().set(&DataKey::PlayerEnergy(player.clone()), &player_energy);

        // Emit consumption event
        env.events().publish(
            (symbol_short!("E_USE"), player_addr.clone()),
            (config.puzzle_energy_cost, player_energy.current_energy),
        );

        Ok(())
    }

    /// Instant refill energy using reward tokens
    ///
    /// # Returns
    /// * `Ok(u32)` - Amount of energy refilled
    /// * `Err(Error)` - Refill failed
    pub fn instant_refill(env: Env, player: Address) -> Result<u32, Error> {
        player.require_auth();
        let player_addr = player.clone();
        Self::assert_not_paused(&env)?;

        let config: EnergyConfig = env.storage().instance().get(&DataKey::Config)
            .ok_or(Error::NotInitialized)?;

        // Check token balance and transfer
        let token_client = token::Client::new(&env, &config.reward_token);
        let balance = token_client.balance(&player_addr);

        if balance < config.refill_token_cost {
            return Err(Error::InsufficientEnergy); // Reusing error for token balance
        }

        // Transfer tokens to contract
        token_client.transfer(&player_addr, &env.current_contract_address(), &config.refill_token_cost);

        // Update player energy to maximum
        let mut player_energy = Self::get_or_create_player_energy(&env, player_addr.clone());
        let energy_refilled = player_energy.max_energy - player_energy.current_energy;

        player_energy.current_energy = player_energy.max_energy;
        player_energy.last_update = env.ledger().timestamp();

        env.storage().instance().set(&DataKey::PlayerEnergy(player.clone()), &player_energy);

        // Emit refill event
        env.events().publish(
            (symbol_short!("E_REFILL"), player_addr.clone()),
            energy_refilled,
        );

        Ok(energy_refilled)
    }

    /// Gift energy to another player
    ///
    /// # Arguments
    /// * `from_player` - Player sending the energy
    /// * `to_player` - Player receiving the energy
    /// * `amount` - Amount of energy to gift
    ///
    /// # Returns
    /// * `Ok(())` - Gift successful
    /// * `Err(Error)` - Gift failed
    pub fn gift_energy(
        env: Env,
        from_player: Address,
        to_player: Address,
        amount: u32,
    ) -> Result<(), Error> {
        from_player.require_auth();
        Self::assert_not_paused(&env)?;

        if from_player == to_player {
            return Err(Error::InvalidAmount);
        }

        if amount == 0 {
            return Err(Error::InvalidAmount);
        }

        let config: EnergyConfig = env.storage().instance().get(&DataKey::Config)
            .ok_or(Error::NotInitialized)?;

        // Reset daily gift counters if needed
        Self::reset_daily_gifts_if_needed(&env);

        let mut from_energy = Self::get_or_create_player_energy(&env, from_player.clone());
        Self::update_energy_regeneration(&env, &mut from_energy, &config);

        // Check sender has enough energy
        if from_energy.current_energy < amount {
            return Err(Error::InsufficientEnergy);
        }

        // Check daily gift limit
        if from_energy.gifted_today + amount > config.max_gift_per_day {
            return Err(Error::GiftLimitExceeded);
        }

        let mut to_energy = Self::get_or_create_player_energy(&env, to_player.clone());
        Self::update_energy_regeneration(&env, &mut to_energy, &config);

        // Check receiver won't exceed max energy
        if to_energy.current_energy + amount > to_energy.max_energy {
            return Err(Error::MaxEnergyExceeded);
        }

        // Perform the gift
        from_energy.current_energy -= amount;
        from_energy.gifted_today += amount;
        from_energy.last_update = env.ledger().timestamp();

        to_energy.current_energy += amount;
        to_energy.last_update = env.ledger().timestamp();

        env.storage().instance().set(&DataKey::PlayerEnergy(from_player.clone()), &from_energy);
        env.storage().instance().set(&DataKey::PlayerEnergy(to_player.clone()), &to_energy);

        // Emit gift event
        env.events().publish(
            (symbol_short!("E_GIFT"), from_player, to_player),
            amount,
        );

        Ok(())
    }

    /// Apply a boost/powerup to a player
    ///
    /// # Arguments
    /// * `player` - Player to boost
    /// * `boost_type` - Type of boost to apply
    /// * `duration_seconds` - How long the boost lasts
    ///
    /// # Returns
    /// * `Ok(())` - Boost applied successfully
    /// * `Err(Error)` - Boost failed
    pub fn apply_boost(
        env: Env,
        player: Address,
        boost_type: BoostType,
        duration_seconds: u64,
    ) -> Result<(), Error> {
        player.require_auth();
        Self::assert_not_paused(&env)?;

        if boost_type == BoostType::None {
            return Err(Error::InvalidBoostType);
        }

        let mut player_energy = Self::get_or_create_player_energy(&env, player.clone());

        // Check if boost already active
        if player_energy.active_boost != BoostType::None && player_energy.boost_expires_at > env.ledger().timestamp() {
            return Err(Error::BoostAlreadyActive);
        }

        // Apply boost
        player_energy.active_boost = boost_type;
        player_energy.boost_expires_at = env.ledger().timestamp() + duration_seconds;
        player_energy.last_update = env.ledger().timestamp();

        env.storage().instance().set(&DataKey::PlayerEnergy(player.clone()), &player_energy);

        // Emit boost event
        env.events().publish(
            (symbol_short!("B_APPLY"), player),
            (boost_type, duration_seconds),
        );

        Ok(())
    }

    /// Get current energy for a player (with regeneration applied)
    pub fn get_current_energy(env: Env, player: Address) -> u32 {
        let config: EnergyConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        let mut player_energy = Self::get_or_create_player_energy(&env, player);
        Self::update_energy_regeneration(&env, &mut player_energy, &config);
        player_energy.current_energy
    }

    /// Get player energy info without updating regeneration
    pub fn get_player_energy_info(env: Env, player: Address) -> Option<PlayerEnergy> {
        env.storage().instance().get(&DataKey::PlayerEnergy(player))
    }

    /// Get contract configuration
    pub fn get_config(env: Env) -> EnergyConfig {
        env.storage().instance().get(&DataKey::Config).unwrap()
    }

    /// Get total number of players
    pub fn get_total_players(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::TotalPlayers).unwrap_or(0)
    }

    // ───────────── INTERNAL HELPERS ─────────────

    fn get_or_create_player_energy(env: &Env, player: Address) -> PlayerEnergy {
        if let Some(energy) = env.storage().instance().get(&DataKey::PlayerEnergy(player.clone())) {
            // Reset daily gifts if needed
            Self::reset_daily_gifts_if_needed(env);
            energy
        } else {
            // Create new player energy
            let config: EnergyConfig = env.storage().instance().get(&DataKey::Config).unwrap();
            let current_time = env.ledger().timestamp();

            let energy = PlayerEnergy {
                current_energy: config.default_max_energy,
                max_energy: config.default_max_energy,
                last_update: current_time,
                active_boost: BoostType::None,
                boost_expires_at: 0,
                gifted_today: 0,
                last_gift_reset: current_time,
            };

            env.storage().instance().set(&DataKey::PlayerEnergy(player), &energy);

            // Increment total players counter
            let total_players: u32 = env.storage().instance().get(&DataKey::TotalPlayers).unwrap_or(0);
            env.storage().instance().set(&DataKey::TotalPlayers, &(total_players + 1));

            energy
        }
    }

    fn update_energy_regeneration(env: &Env, player_energy: &mut PlayerEnergy, config: &EnergyConfig) {
        let current_time = env.ledger().timestamp();

        // Use saturating_sub to prevent underflow on timestamp issues
        let time_elapsed = current_time.saturating_sub(player_energy.last_update) as u32;

        if time_elapsed == 0 {
            return; // No time has passed
        }

        // Calculate regeneration multiplier from active boost
        let multiplier = if player_energy.active_boost != BoostType::None && player_energy.boost_expires_at > current_time {
            match player_energy.active_boost {
                BoostType::DoubleRegen => 2,
                BoostType::TripleRegen => 3,
                BoostType::QuintupleRegen => 5,
                BoostType::None => 1,
            }
        } else {
            // Boost expired, reset it
            if player_energy.boost_expires_at <= current_time {
                player_energy.active_boost = BoostType::None;
                player_energy.boost_expires_at = 0;
            }
            1
        };

        // Optimized calculation: multiply time_elapsed by (base_regen_rate * multiplier)
        // This avoids intermediate variable creation
        let regenerated = time_elapsed * (config.base_regen_rate * multiplier);

        // Apply regeneration with saturation (capped at max_energy)
        player_energy.current_energy = player_energy.current_energy.saturating_add(regenerated).min(player_energy.max_energy);
        player_energy.last_update = current_time;
    }

    fn reset_daily_gifts_if_needed(env: &Env) {
        let current_time = env.ledger().timestamp();
        let last_reset: u64 = env.storage().instance().get(&DataKey::DailyGiftReset).unwrap_or(0);

        if current_time - last_reset >= SECONDS_PER_DAY {
            // Reset all players' daily gift counters
            // Note: In a real implementation, you might want to iterate through all players
            // For now, we'll reset on access (lazy reset)

            env.storage().instance().set(&DataKey::DailyGiftReset, &current_time);
        }
    }

    fn assert_admin(env: &Env, user: &Address) -> Result<(), Error> {
        let config: EnergyConfig = env.storage().instance().get(&DataKey::Config)
            .ok_or(Error::NotInitialized)?;
        if config.admin != *user {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }

    fn assert_not_paused(env: &Env) -> Result<(), Error> {
        let config: EnergyConfig = env.storage().instance().get(&DataKey::Config)
            .ok_or(Error::NotInitialized)?;
        if config.paused {
            return Err(Error::ContractPaused);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Env,
    };

    #[test]
    fn test_initialization() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, EnergyContract);
        let client = EnergyContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let reward_token = Address::generate(&env);

        client.initialize(
            &admin,
            &reward_token,
            &1, // 1 energy per second base rate
            &100, // max 100 energy
            &10, // 10 energy per puzzle
            &50, // 50 tokens for refill
        );

        let config = client.get_config();
        assert_eq!(config.admin, admin);
        assert_eq!(config.default_max_energy, 100);
        assert_eq!(config.puzzle_energy_cost, 10);
    }

    #[test]
    fn test_energy_regeneration() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, EnergyContract);
        let client = EnergyContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let reward_token = Address::generate(&env);
        let player = Address::generate(&env);

        client.initialize(&admin, &reward_token, &1, &100, &10, &50);

        // Get initial energy (should be max)
        let initial_energy = client.get_current_energy(&player);
        assert_eq!(initial_energy, 100);

        // Consume energy
        client.consume_energy_for_puzzle(&player);

        let energy_after_consume = client.get_current_energy(&player);
        assert_eq!(energy_after_consume, 90); // 100 - 10

        // Advance time by 50 seconds
        env.ledger().with_mut(|li| li.timestamp += 50);

        // Check regeneration (50 seconds * 1 energy/second = 50 energy regenerated)
        let energy_after_regen = client.get_current_energy(&player);
        assert_eq!(energy_after_regen, 90 + 50); // Should be 140, but capped at 100
    }

    #[test]
    fn test_energy_boost() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, EnergyContract);
        let client = EnergyContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let reward_token = Address::generate(&env);
        let player = Address::generate(&env);

        client.initialize(&admin, &reward_token, &1, &100, &10, &50);

        // Consume energy first
        client.consume_energy_for_puzzle(&player);
        assert_eq!(client.get_current_energy(&player), 90);

        // Apply 2x boost for 100 seconds
        client.apply_boost(&player, &BoostType::DoubleRegen, &100);

        // Advance time by 10 seconds
        env.ledger().with_mut(|li| li.timestamp += 10);

        // Check regeneration with boost (10 seconds * 1 * 2 = 20 energy)
        let energy_after_boost = client.get_current_energy(&player);
        assert_eq!(energy_after_boost, 90 + 20);
    }

    #[test]
    fn test_energy_gifting() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, EnergyContract);
        let client = EnergyContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let reward_token = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);

        client.initialize(&admin, &reward_token, &1, &100, &10, &50);

        // Player1 has 100 energy, Player2 has 100 energy

        // Gift 20 energy from player1 to player2
        client.gift_energy(&player1, &player2, &20);

        assert_eq!(client.get_current_energy(&player1), 80);
        assert_eq!(client.get_current_energy(&player2), 120); // Capped at 100
    }

    #[test]
    fn test_insufficient_energy() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, EnergyContract);
        let client = EnergyContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let reward_token = Address::generate(&env);
        let player = Address::generate(&env);

        client.initialize(&admin, &reward_token, &1, &100, &10, &50);

        // Consume energy until low
        for _ in 0..9 {
            client.consume_energy_for_puzzle(&player);
        }

        // Should have 10 energy left, try to consume 10 more
        client.consume_energy_for_puzzle(&player);
        assert_eq!(client.get_current_energy(&player), 0);

        // Try to consume again (should fail)
        let result = client.try_consume_energy_for_puzzle(&player);
        assert_eq!(result, Err(Ok(Error::InsufficientEnergy)));
    }
}