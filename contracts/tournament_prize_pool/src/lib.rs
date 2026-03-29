#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env, Vec,
};

// ──────────────────────────────────────────────────────────
// ERROR CODES
// ──────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PoolError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    TournamentNotFound = 4,
    InvalidTierSplits = 5,
    InvalidAmount = 6,
    WrongStatus = 7,
    StandingsAlreadySubmitted = 8,
    NotDistributed = 9,
    AlreadyDistributed = 10,
    StandingsMismatch = 11,
}

// ──────────────────────────────────────────────────────────
// DATA KEYS
// ──────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Oracle,
    Token,
    NextTournamentId,
    Tournament(u32),
    Standings(u32),
}

// ──────────────────────────────────────────────────────────
// STRUCTS
// ──────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TournamentStatus {
    Locked = 0,
    StandingsSubmitted = 1,
    Distributed = 2,
    Cancelled = 3,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct TournamentPool {
    pub tournament_id: u32,
    pub organiser: Address,
    pub total_fund: i128,
    pub status: TournamentStatus,
    pub tier_splits: Vec<u32>,
    pub distributed: bool,
}

// ──────────────────────────────────────────────────────────
// CONTRACT
// ──────────────────────────────────────────────────────────

#[contract]
pub struct TournamentPrizePoolContract;

#[contractimpl]
impl TournamentPrizePoolContract {
    /// Initialize the contract with admin, oracle, and prize token address.
    pub fn initialize(
        env: Env,
        admin: Address,
        oracle: Address,
        token: Address,
    ) -> Result<(), PoolError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PoolError::AlreadyInitialized);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Oracle, &oracle);
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::NextTournamentId, &0u32);

        Ok(())
    }

    // ───────────── ORGANISER FUNCTIONS ─────────────

    /// Lock a prize fund for a tournament. Organiser deposits tokens and sets tier splits.
    /// `tier_splits` is a Vec of basis points that must sum to 10000 (100%).
    pub fn lock_fund(
        env: Env,
        organiser: Address,
        amount: i128,
        tier_splits: Vec<u32>,
    ) -> Result<u32, PoolError> {
        organiser.require_auth();

        if amount <= 0 {
            return Err(PoolError::InvalidAmount);
        }

        // Validate tier_splits sum to 10000 bps
        Self::validate_splits(&tier_splits)?;

        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .ok_or(PoolError::NotInitialized)?;

        // Transfer tokens from organiser to contract
        let token_client = token::Client::new(&env, &token_addr);
        token_client.transfer(&organiser, &env.current_contract_address(), &amount);

        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextTournamentId)
            .unwrap_or(0);

        let pool = TournamentPool {
            tournament_id: id,
            organiser: organiser.clone(),
            total_fund: amount,
            status: TournamentStatus::Locked,
            tier_splits,
            distributed: false,
        };

        env.storage().persistent().set(&DataKey::Tournament(id), &pool);
        env.storage().instance().set(&DataKey::NextTournamentId, &(id + 1));

        env.events().publish(
            (symbol_short!("locked"), organiser),
            (id, amount),
        );

        Ok(id)
    }

    /// Cancel a tournament and refund the organiser. Only allowed while status is Locked
    /// (before standings are submitted).
    pub fn cancel(env: Env, tournament_id: u32) -> Result<(), PoolError> {
        let mut pool = Self::get_pool_or_err(&env, tournament_id)?;

        if pool.status != TournamentStatus::Locked {
            return Err(PoolError::WrongStatus);
        }

        pool.organiser.require_auth();

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let token_client = token::Client::new(&env, &token_addr);

        // Refund the full fund to organiser
        token_client.transfer(
            &env.current_contract_address(),
            &pool.organiser,
            &pool.total_fund,
        );

        pool.status = TournamentStatus::Cancelled;
        env.storage().persistent().set(&DataKey::Tournament(tournament_id), &pool);

        env.events().publish(
            (symbol_short!("cancel"), pool.organiser.clone()),
            tournament_id,
        );

        Ok(())
    }

    // ───────────── ORACLE FUNCTIONS ─────────────

    /// Submit final standings for a tournament. Only callable by oracle.
    /// `ranked_players` is ordered: index 0 = 1st place, index 1 = 2nd, etc.
    /// Length must match tier_splits length.
    pub fn submit_standings(
        env: Env,
        tournament_id: u32,
        ranked_players: Vec<Address>,
    ) -> Result<(), PoolError> {
        let oracle: Address = env
            .storage()
            .instance()
            .get(&DataKey::Oracle)
            .ok_or(PoolError::NotInitialized)?;
        oracle.require_auth();

        let mut pool = Self::get_pool_or_err(&env, tournament_id)?;

        if pool.status != TournamentStatus::Locked {
            return Err(PoolError::WrongStatus);
        }

        if ranked_players.len() != pool.tier_splits.len() {
            return Err(PoolError::StandingsMismatch);
        }

        pool.status = TournamentStatus::StandingsSubmitted;
        env.storage().persistent().set(&DataKey::Tournament(tournament_id), &pool);
        env.storage()
            .persistent()
            .set(&DataKey::Standings(tournament_id), &ranked_players);

        env.events().publish(
            (symbol_short!("standng"), oracle),
            tournament_id,
        );

        Ok(())
    }

    // ───────────── DISTRIBUTION ─────────────

    /// Distribute prize fund to ranked players according to tier splits.
    /// Callable by anyone after standings have been submitted.
    pub fn distribute(env: Env, tournament_id: u32) -> Result<(), PoolError> {
        let mut pool = Self::get_pool_or_err(&env, tournament_id)?;

        if pool.status != TournamentStatus::StandingsSubmitted {
            return Err(PoolError::WrongStatus);
        }

        let ranked_players: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Standings(tournament_id))
            .ok_or(PoolError::WrongStatus)?;

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let token_client = token::Client::new(&env, &token_addr);

        let total = pool.total_fund;

        for i in 0..pool.tier_splits.len() {
            let bps = pool.tier_splits.get(i).unwrap();
            let player = ranked_players.get(i).unwrap();
            let share = (total * bps as i128) / 10_000i128;

            if share > 0 {
                token_client.transfer(
                    &env.current_contract_address(),
                    &player,
                    &share,
                );

                env.events().publish(
                    (symbol_short!("prize"), player),
                    (tournament_id, share),
                );
            }
        }

        pool.status = TournamentStatus::Distributed;
        pool.distributed = true;
        env.storage().persistent().set(&DataKey::Tournament(tournament_id), &pool);

        Ok(())
    }

    // ───────────── ADMIN FUNCTIONS ─────────────

    /// Update the oracle address. Admin only.
    pub fn set_oracle(env: Env, new_oracle: Address) -> Result<(), PoolError> {
        Self::require_admin(&env)?;
        env.storage().instance().set(&DataKey::Oracle, &new_oracle);
        Ok(())
    }

    // ───────────── VIEW FUNCTIONS ─────────────

    /// Get tournament pool info.
    pub fn get_pool(env: Env, tournament_id: u32) -> Result<TournamentPool, PoolError> {
        Self::get_pool_or_err(&env, tournament_id)
    }

    /// Get submitted standings for a tournament.
    pub fn get_standings(env: Env, tournament_id: u32) -> Result<Vec<Address>, PoolError> {
        env.storage()
            .persistent()
            .get(&DataKey::Standings(tournament_id))
            .ok_or(PoolError::TournamentNotFound)
    }

    // ───────────── INTERNAL HELPERS ─────────────

    fn require_admin(env: &Env) -> Result<(), PoolError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(PoolError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }

    fn get_pool_or_err(env: &Env, tournament_id: u32) -> Result<TournamentPool, PoolError> {
        env.storage()
            .persistent()
            .get::<DataKey, TournamentPool>(&DataKey::Tournament(tournament_id))
            .ok_or(PoolError::TournamentNotFound)
    }

    fn validate_splits(splits: &Vec<u32>) -> Result<(), PoolError> {
        if splits.is_empty() {
            return Err(PoolError::InvalidTierSplits);
        }
        let mut total: u32 = 0;
        for i in 0..splits.len() {
            total += splits.get(i).unwrap();
        }
        if total != 10_000 {
            return Err(PoolError::InvalidTierSplits);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test;
