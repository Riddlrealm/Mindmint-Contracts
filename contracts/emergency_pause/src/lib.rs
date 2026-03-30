#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Env, String, Symbol, Vec,
};

// ─── Errors ───────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PauseError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    AlreadyPaused = 4,
    NotPaused = 5,
    UnpauseNotRequested = 6,
    TimelockNotExpired = 7,
    GuardianAlreadyExists = 8,
    GuardianNotFound = 9,
    InvalidTimelock = 10,
}

// ─── Types ────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseState {
    pub paused: bool,
    pub paused_at: u64,
    pub paused_by: Address,
    pub reason: String,
    pub unpause_after: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseEvent {
    pub action: Symbol,
    pub actor: Address,
    pub reason: String,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Guardians,
    PauseState,
    UnpauseTimelock,
    PauseHistory,
}

// ─── Default timelock: 24h in seconds ─────────────────────
const DEFAULT_TIMELOCK: u64 = 86_400;

// ─── Contract ─────────────────────────────────────────────

#[contract]
pub struct EmergencyPauseContract;

#[contractimpl]
impl EmergencyPauseContract {
    /// Initialize with admin. Admin is also the first guardian.
    pub fn initialize(env: Env, admin: Address) -> Result<(), PauseError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PauseError::AlreadyInitialized);
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);

        let mut guardians = Vec::<Address>::new(&env);
        guardians.push_back(admin.clone());
        env.storage().instance().set(&DataKey::Guardians, &guardians);

        env.storage()
            .instance()
            .set(&DataKey::UnpauseTimelock, &DEFAULT_TIMELOCK);

        let history = Vec::<PauseEvent>::new(&env);
        env.storage().persistent().set(&DataKey::PauseHistory, &history);

        Ok(())
    }

    // ─── Pause / Unpause ──────────────────────────────────

    /// Guardian pauses the contract with a reason.
    pub fn pause(env: Env, guardian: Address, reason: String) -> Result<(), PauseError> {
        guardian.require_auth();
        Self::require_initialized(&env)?;
        Self::require_guardian(&env, &guardian)?;

        if Self::is_currently_paused(&env) {
            return Err(PauseError::AlreadyPaused);
        }

        let now = env.ledger().timestamp();

        let state = PauseState {
            paused: true,
            paused_at: now,
            paused_by: guardian.clone(),
            reason: reason.clone(),
            unpause_after: 0,
        };
        env.storage().instance().set(&DataKey::PauseState, &state);

        // Record history
        Self::record_event(
            &env,
            Symbol::new(&env, "paused"),
            guardian.clone(),
            reason.clone(),
            now,
        );

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "ContractPaused"),),
            (guardian, reason),
        );

        Ok(())
    }

    /// Guardian requests unpause — starts the timelock countdown.
    pub fn request_unpause(env: Env, guardian: Address) -> Result<u64, PauseError> {
        guardian.require_auth();
        Self::require_initialized(&env)?;
        Self::require_guardian(&env, &guardian)?;

        if !Self::is_currently_paused(&env) {
            return Err(PauseError::NotPaused);
        }

        let now = env.ledger().timestamp();
        let timelock: u64 = env
            .storage()
            .instance()
            .get(&DataKey::UnpauseTimelock)
            .unwrap();
        let unpause_after = now + timelock;

        let mut state: PauseState = env
            .storage()
            .instance()
            .get(&DataKey::PauseState)
            .unwrap();
        state.unpause_after = unpause_after;
        env.storage().instance().set(&DataKey::PauseState, &state);

        // Record history
        let reason = String::from_str(&env, "unpause_requested");
        Self::record_event(
            &env,
            Symbol::new(&env, "unpause_req"),
            guardian.clone(),
            reason,
            now,
        );

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "UnpauseRequested"),),
            unpause_after,
        );

        Ok(unpause_after)
    }

    /// Anyone can execute unpause after the timelock has expired.
    pub fn execute_unpause(env: Env) -> Result<(), PauseError> {
        Self::require_initialized(&env)?;

        if !Self::is_currently_paused(&env) {
            return Err(PauseError::NotPaused);
        }

        let state: PauseState = env
            .storage()
            .instance()
            .get(&DataKey::PauseState)
            .unwrap();

        if state.unpause_after == 0 {
            return Err(PauseError::UnpauseNotRequested);
        }

        let now = env.ledger().timestamp();
        if now < state.unpause_after {
            return Err(PauseError::TimelockNotExpired);
        }

        // Clear pause state
        env.storage().instance().remove(&DataKey::PauseState);

        // Record history
        let reason = String::from_str(&env, "unpaused");
        Self::record_event(
            &env,
            Symbol::new(&env, "unpaused"),
            env.current_contract_address(),
            reason,
            now,
        );

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "ContractUnpaused"),),
            now,
        );

        Ok(())
    }

    /// Query: returns true if contract is currently paused.
    pub fn is_paused(env: Env) -> bool {
        Self::is_currently_paused(&env)
    }

    /// Query: returns current pause state (panics if not paused).
    pub fn get_pause_state(env: Env) -> Result<PauseState, PauseError> {
        Self::require_initialized(&env)?;
        env.storage()
            .instance()
            .get(&DataKey::PauseState)
            .ok_or(PauseError::NotPaused)
    }

    // ─── Guardian Management ──────────────────────────────

    /// Admin adds a new guardian.
    pub fn add_guardian(env: Env, new_guardian: Address) -> Result<(), PauseError> {
        Self::require_admin(&env)?;

        let mut guardians: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Guardians)
            .unwrap();

        // Check duplicate
        for i in 0..guardians.len() {
            if guardians.get(i).unwrap() == new_guardian {
                return Err(PauseError::GuardianAlreadyExists);
            }
        }

        guardians.push_back(new_guardian);
        env.storage()
            .instance()
            .set(&DataKey::Guardians, &guardians);

        Ok(())
    }

    /// Admin removes a guardian.
    pub fn remove_guardian(env: Env, guardian: Address) -> Result<(), PauseError> {
        Self::require_admin(&env)?;

        let guardians: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Guardians)
            .unwrap();

        let mut found = false;
        let mut new_guardians = Vec::<Address>::new(&env);
        for i in 0..guardians.len() {
            let g = guardians.get(i).unwrap();
            if g == guardian {
                found = true;
            } else {
                new_guardians.push_back(g);
            }
        }

        if !found {
            return Err(PauseError::GuardianNotFound);
        }

        env.storage()
            .instance()
            .set(&DataKey::Guardians, &new_guardians);

        Ok(())
    }

    /// Query: returns all guardians.
    pub fn get_guardians(env: Env) -> Result<Vec<Address>, PauseError> {
        Self::require_initialized(&env)?;
        Ok(env
            .storage()
            .instance()
            .get(&DataKey::Guardians)
            .unwrap())
    }

    // ─── Timelock Configuration ───────────────────────────

    /// Admin configures the unpause timelock duration (in seconds).
    pub fn set_timelock(env: Env, duration: u64) -> Result<(), PauseError> {
        Self::require_admin(&env)?;

        if duration == 0 {
            return Err(PauseError::InvalidTimelock);
        }

        env.storage()
            .instance()
            .set(&DataKey::UnpauseTimelock, &duration);

        Ok(())
    }

    /// Query: returns current timelock duration.
    pub fn get_timelock(env: Env) -> Result<u64, PauseError> {
        Self::require_initialized(&env)?;
        Ok(env
            .storage()
            .instance()
            .get(&DataKey::UnpauseTimelock)
            .unwrap())
    }

    // ─── History ──────────────────────────────────────────

    /// Returns the full pause/unpause event history.
    pub fn get_pause_history(env: Env) -> Result<Vec<PauseEvent>, PauseError> {
        Self::require_initialized(&env)?;
        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::PauseHistory)
            .unwrap_or(Vec::new(&env)))
    }

    // ─── Internal Helpers ─────────────────────────────────

    fn require_initialized(env: &Env) -> Result<(), PauseError> {
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(PauseError::NotInitialized);
        }
        Ok(())
    }

    fn require_admin(env: &Env) -> Result<(), PauseError> {
        Self::require_initialized(env)?;
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        Ok(())
    }

    fn require_guardian(env: &Env, addr: &Address) -> Result<(), PauseError> {
        let guardians: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Guardians)
            .unwrap();
        for i in 0..guardians.len() {
            if &guardians.get(i).unwrap() == addr {
                return Ok(());
            }
        }
        Err(PauseError::Unauthorized)
    }

    fn is_currently_paused(env: &Env) -> bool {
        env.storage()
            .instance()
            .get::<_, PauseState>(&DataKey::PauseState)
            .map(|s| s.paused)
            .unwrap_or(false)
    }

    fn record_event(env: &Env, action: Symbol, actor: Address, reason: String, timestamp: u64) {
        let mut history: Vec<PauseEvent> = env
            .storage()
            .persistent()
            .get(&DataKey::PauseHistory)
            .unwrap_or(Vec::new(env));

        history.push_back(PauseEvent {
            action,
            actor,
            reason,
            timestamp,
        });

        env.storage()
            .persistent()
            .set(&DataKey::PauseHistory, &history);
    }
}

#[cfg(test)]
mod test;
