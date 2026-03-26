#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env,
    Symbol,
};

const DEFAULT_FEE_BPS: u32 = 200;
const MAX_FEE_BPS: u32 = 10_000;
const ACCEPTANCE_WINDOW_SECS: u64 = 24 * 60 * 60;

const EVENT_WAGER: Symbol = symbol_short!("wager");
const EVENT_CREATE: Symbol = symbol_short!("create");
const EVENT_ACCEPT: Symbol = symbol_short!("accept");
const EVENT_DECLINE: Symbol = symbol_short!("decline");
const EVENT_RESULT: Symbol = symbol_short!("result");
const EVENT_CLAIM: Symbol = symbol_short!("claim");
const EVENT_CANCEL: Symbol = symbol_short!("cancel");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WagerType {
    Speed,
    Score,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WagerStatus {
    Pending,
    Active,
    Declined,
    ResultSubmitted,
    Claimed,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SocialWager {
    pub wager_id: u64,
    pub challenger: Address,
    pub opponent: Address,
    pub puzzle_id: u32,
    pub stake_amount: i128,
    pub wager_type: WagerType,
    pub status: WagerStatus,
    pub winner: Option<Address>,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub token: Address,
    pub oracle: Address,
    pub fee_bps: u32,
    pub fee_recipient: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Config,
    NextWagerId,
    Wager(u64),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SocialWagerError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidStakeAmount = 3,
    InvalidOpponent = 4,
    WagerNotFound = 5,
    InvalidStatus = 6,
    Unauthorized = 7,
    WagerExpired = 8,
    InvalidWinner = 9,
    WinnerNotSet = 10,
    InvalidFeeBps = 11,
}

#[contract]
pub struct SocialWagerContract;

#[contractimpl]
impl SocialWagerContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        token: Address,
        oracle: Address,
        fee_recipient: Option<Address>,
    ) -> Result<(), SocialWagerError> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(SocialWagerError::AlreadyInitialized);
        }

        let config = Config {
            admin: admin.clone(),
            token,
            oracle,
            fee_bps: DEFAULT_FEE_BPS,
            fee_recipient: fee_recipient.unwrap_or(admin),
        };

        env.storage().instance().set(&DataKey::Config, &config);
        env.storage().instance().set(&DataKey::NextWagerId, &1u64);

        Ok(())
    }

    pub fn set_fee_bps(env: Env, admin: Address, fee_bps: u32) -> Result<(), SocialWagerError> {
        admin.require_auth();

        if fee_bps > MAX_FEE_BPS {
            return Err(SocialWagerError::InvalidFeeBps);
        }

        let mut config = Self::get_config(env.clone())?;
        if admin != config.admin {
            return Err(SocialWagerError::Unauthorized);
        }

        config.fee_bps = fee_bps;
        env.storage().instance().set(&DataKey::Config, &config);
        Ok(())
    }

    pub fn set_oracle(
        env: Env,
        admin: Address,
        oracle: Address,
    ) -> Result<(), SocialWagerError> {
        admin.require_auth();

        let mut config = Self::get_config(env.clone())?;
        if admin != config.admin {
            return Err(SocialWagerError::Unauthorized);
        }

        config.oracle = oracle;
        env.storage().instance().set(&DataKey::Config, &config);
        Ok(())
    }

    pub fn create_wager(
        env: Env,
        challenger: Address,
        opponent: Address,
        puzzle_id: u32,
        stake_amount: i128,
        wager_type: WagerType,
    ) -> Result<u64, SocialWagerError> {
        challenger.require_auth();

        if challenger == opponent {
            return Err(SocialWagerError::InvalidOpponent);
        }
        if stake_amount <= 0 {
            return Err(SocialWagerError::InvalidStakeAmount);
        }

        let config = Self::get_config(env.clone())?;
        let wager_id = Self::next_wager_id(&env);
        let wager = SocialWager {
            wager_id,
            challenger: challenger.clone(),
            opponent: opponent.clone(),
            puzzle_id,
            stake_amount,
            wager_type,
            status: WagerStatus::Pending,
            winner: None,
            created_at: env.ledger().timestamp(),
        };

        token::Client::new(&env, &config.token).transfer(
            &challenger,
            &env.current_contract_address(),
            &stake_amount,
        );

        env.storage().persistent().set(&DataKey::Wager(wager_id), &wager);
        env.storage()
            .instance()
            .set(&DataKey::NextWagerId, &(wager_id + 1));

        env.events().publish(
            (EVENT_WAGER, EVENT_CREATE),
            (wager_id, challenger, opponent, puzzle_id, stake_amount),
        );

        Ok(wager_id)
    }

    pub fn accept_wager(
        env: Env,
        opponent: Address,
        wager_id: u64,
    ) -> Result<SocialWager, SocialWagerError> {
        let mut wager = Self::get_wager_internal(&env, wager_id)?;
        if Self::cancel_if_expired(&env, &mut wager)? {
            return Err(SocialWagerError::WagerExpired);
        }

        opponent.require_auth();

        if wager.status != WagerStatus::Pending {
            return Err(SocialWagerError::InvalidStatus);
        }
        if opponent != wager.opponent {
            return Err(SocialWagerError::Unauthorized);
        }

        let config = Self::get_config(env.clone())?;
        token::Client::new(&env, &config.token).transfer(
            &opponent,
            &env.current_contract_address(),
            &wager.stake_amount,
        );

        wager.status = WagerStatus::Active;
        env.storage().persistent().set(&DataKey::Wager(wager_id), &wager);
        env.events()
            .publish((EVENT_WAGER, EVENT_ACCEPT), (wager_id, opponent));

        Ok(wager)
    }

    pub fn decline_wager(
        env: Env,
        opponent: Address,
        wager_id: u64,
    ) -> Result<SocialWager, SocialWagerError> {
        let mut wager = Self::get_wager_internal(&env, wager_id)?;
        if Self::cancel_if_expired(&env, &mut wager)? {
            return Err(SocialWagerError::WagerExpired);
        }

        opponent.require_auth();

        if wager.status != WagerStatus::Pending {
            return Err(SocialWagerError::InvalidStatus);
        }
        if opponent != wager.opponent {
            return Err(SocialWagerError::Unauthorized);
        }

        let config = Self::get_config(env.clone())?;
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &wager.challenger,
            &wager.stake_amount,
        );

        wager.status = WagerStatus::Declined;
        env.storage().persistent().set(&DataKey::Wager(wager_id), &wager);
        env.events()
            .publish((EVENT_WAGER, EVENT_DECLINE), (wager_id, opponent));

        Ok(wager)
    }

    pub fn submit_result(
        env: Env,
        oracle: Address,
        wager_id: u64,
        winner: Address,
    ) -> Result<SocialWager, SocialWagerError> {
        oracle.require_auth();

        let config = Self::get_config(env.clone())?;
        if oracle != config.oracle {
            return Err(SocialWagerError::Unauthorized);
        }

        let mut wager = Self::get_wager_internal(&env, wager_id)?;
        if wager.status != WagerStatus::Active {
            return Err(SocialWagerError::InvalidStatus);
        }
        if winner != wager.challenger && winner != wager.opponent {
            return Err(SocialWagerError::InvalidWinner);
        }

        wager.winner = Some(winner.clone());
        wager.status = WagerStatus::ResultSubmitted;
        env.storage().persistent().set(&DataKey::Wager(wager_id), &wager);
        env.events()
            .publish((EVENT_WAGER, EVENT_RESULT), (wager_id, winner));

        Ok(wager)
    }

    pub fn claim_winnings(
        env: Env,
        winner: Address,
        wager_id: u64,
    ) -> Result<SocialWager, SocialWagerError> {
        let mut wager = Self::get_wager_internal(&env, wager_id)?;
        if wager.status != WagerStatus::ResultSubmitted {
            return Err(SocialWagerError::InvalidStatus);
        }

        winner.require_auth();

        let stored_winner = wager
            .winner
            .clone()
            .ok_or(SocialWagerError::WinnerNotSet)?;
        if winner != stored_winner {
            return Err(SocialWagerError::Unauthorized);
        }

        let config = Self::get_config(env.clone())?;
        let total_pot = wager.stake_amount * 2;
        let fee_amount = total_pot * (config.fee_bps as i128) / 10_000;
        let payout_amount = total_pot - fee_amount;
        let token_client = token::Client::new(&env, &config.token);

        if payout_amount > 0 {
            token_client.transfer(&env.current_contract_address(), &winner, &payout_amount);
        }
        if fee_amount > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &config.fee_recipient,
                &fee_amount,
            );
        }

        wager.status = WagerStatus::Claimed;
        env.storage().persistent().set(&DataKey::Wager(wager_id), &wager);
        env.events()
            .publish((EVENT_WAGER, EVENT_CLAIM), (wager_id, winner, payout_amount, fee_amount));

        Ok(wager)
    }

    pub fn get_wager(env: Env, wager_id: u64) -> Result<SocialWager, SocialWagerError> {
        let mut wager = Self::get_wager_internal(&env, wager_id)?;
        Self::cancel_if_expired(&env, &mut wager)?;
        Ok(wager)
    }

    pub fn get_config(env: Env) -> Result<Config, SocialWagerError> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(SocialWagerError::NotInitialized)
    }

    fn get_wager_internal(env: &Env, wager_id: u64) -> Result<SocialWager, SocialWagerError> {
        env.storage()
            .persistent()
            .get(&DataKey::Wager(wager_id))
            .ok_or(SocialWagerError::WagerNotFound)
    }

    fn next_wager_id(env: &Env) -> u64 {
        env.storage().instance().get(&DataKey::NextWagerId).unwrap_or(1)
    }

    fn cancel_if_expired(
        env: &Env,
        wager: &mut SocialWager,
    ) -> Result<bool, SocialWagerError> {
        if wager.status != WagerStatus::Pending {
            return Ok(false);
        }

        let now = env.ledger().timestamp();
        if now <= wager.created_at + ACCEPTANCE_WINDOW_SECS {
            return Ok(false);
        }

        let config = Self::get_config(env.clone())?;
        token::Client::new(env, &config.token).transfer(
            &env.current_contract_address(),
            &wager.challenger,
            &wager.stake_amount,
        );

        wager.status = WagerStatus::Cancelled;
        env.storage()
            .persistent()
            .set(&DataKey::Wager(wager.wager_id), wager);
        env.events()
            .publish((EVENT_WAGER, EVENT_CANCEL), (wager.wager_id, wager.challenger.clone()));

        Ok(true)
    }
}

#[cfg(test)]
mod test;
