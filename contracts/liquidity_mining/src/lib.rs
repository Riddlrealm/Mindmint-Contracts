#![no_std]

mod storage;
#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, Address, Env, symbol_short};
use soroban_sdk::token::Client as TokenClient;
use crate::storage::*;

#[contract]
pub struct LiquidityMiningContract;

#[contractimpl]
impl LiquidityMiningContract {
    pub fn initialize(env: Env, admin: Address, lp_token: Address, reward_token: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        set_admin(&env, &admin);
        set_lp_token(&env, &lp_token);
        set_reward_token(&env, &reward_token);
    }

    pub fn fund_and_open_epoch(env: Env, reward_budget: i128) -> u32 {
        let admin = get_admin(&env);
        admin.require_auth();

        if reward_budget <= 0 {
            panic!("Invalid budget");
        }

        let current_id = get_current_epoch_id(&env);
        if current_id > 0 {
            let last_epoch = get_epoch(&env, current_id).expect("Epoch not found");
            if last_epoch.end_at == 0 {
                panic!("Previous epoch still open");
            }
        }

        // Fund from admin
        let reward_token = get_reward_token(&env);
        let token = TokenClient::new(&env, &reward_token);
        token.transfer(&admin, &env.current_contract_address(), &reward_budget);

        let new_id = current_id + 1;
        let epoch = MiningEpoch {
            epoch_id: new_id,
            start_at: env.ledger().timestamp(),
            end_at: 0, // 0 signifies it's open
            reward_budget,
            total_lp_staked: 0,
            distributed: false,
        };

        set_epoch(&env, new_id, &epoch);
        set_current_epoch_id(&env, new_id);

        new_id
    }

    pub fn close_epoch(env: Env) -> u32 {
        let admin = get_admin(&env);
        admin.require_auth();

        let current_id = get_current_epoch_id(&env);
        if current_id == 0 {
            panic!("No active epoch");
        }

        let mut epoch = get_epoch(&env, current_id).expect("Epoch not found");
        if epoch.end_at != 0 {
            panic!("Epoch already closed");
        }

        // Snapshot total LP staked at closing time
        epoch.end_at = env.ledger().timestamp();
        epoch.total_lp_staked = get_global_lp_staked(&env);
        
        set_epoch(&env, current_id, &epoch);
        
        current_id
    }

    pub fn stake_lp(env: Env, provider: Address, amount: i128) {
        provider.require_auth();

        if amount <= 0 {
            panic!("Invalid amount");
        }

        Self::require_no_pending_claims(&env, &provider);

        let lp_token = get_lp_token(&env);
        let client = TokenClient::new(&env, &lp_token);
        client.transfer(&provider, &env.current_contract_address(), &amount);

        let mut pos = get_position(&env, &provider);
        pos.lp_tokens += amount;
        pos.staked_at = env.ledger().timestamp(); // Reset cooldown
        set_position(&env, &provider, &pos);

        let global_staked = get_global_lp_staked(&env);
        set_global_lp_staked(&env, global_staked + amount);

        env.events().publish((symbol_short!("Stake"), symbol_short!("LP")), (provider, amount));
    }

    pub fn unstake_lp(env: Env, provider: Address, amount: i128) {
        provider.require_auth();

        if amount <= 0 {
            panic!("Invalid amount");
        }

        let mut pos = get_position(&env, &provider);
        if pos.lp_tokens < amount {
            panic!("Insufficient staked balance");
        }

        let now = env.ledger().timestamp();
        // 24 hour cooldown = 86400 seconds
        if now < pos.staked_at + 86400 {
            panic!("Unstake cooldown active");
        }

        Self::require_no_pending_claims(&env, &provider);

        pos.lp_tokens -= amount;
        set_position(&env, &provider, &pos);

        let global_staked = get_global_lp_staked(&env);
        set_global_lp_staked(&env, global_staked - amount);

        let lp_token = get_lp_token(&env);
        let client = TokenClient::new(&env, &lp_token);
        client.transfer(&env.current_contract_address(), &provider, &amount);

        env.events().publish((symbol_short!("Unstake"), symbol_short!("LP")), (provider, amount));
    }

    pub fn claim_mining_reward(env: Env, provider: Address, epoch_id: u32) -> i128 {
        provider.require_auth();

        let epoch = get_epoch(&env, epoch_id).expect("Epoch not found");
        if epoch.end_at == 0 {
            panic!("Epoch not closed");
        }

        if has_claimed(&env, &provider, epoch_id) {
            panic!("Already claimed for epoch");
        }

        let mut pos = get_position(&env, &provider);
        if pos.lp_tokens == 0 {
            // Cannot earn if you had no tokens during the epoch tracking.
            // Notice: since we require pending claims to be zero before modifying stake, 
            // pos.lp_tokens is exactly what they had when the epoch closed.
            return 0;
        }

        if pos.last_claim_epoch >= epoch_id {
            panic!("Invalid epoch order");
        }

        let mut reward = 0;
        if epoch.total_lp_staked > 0 {
            // Check precision limits in production. Safe enough for general tokens.
            reward = (pos.lp_tokens as i128 * epoch.reward_budget as i128) / epoch.total_lp_staked as i128;
        }

        set_has_claimed(&env, &provider, epoch_id);
        pos.last_claim_epoch = epoch_id;
        pos.total_claimed += reward;
        set_position(&env, &provider, &pos);

        if reward > 0 {
            let reward_token = get_reward_token(&env);
            let client = TokenClient::new(&env, &reward_token);
            client.transfer(&env.current_contract_address(), &provider, &reward);
        }

        env.events().publish((symbol_short!("Claim"), symbol_short!("Reward")), (provider, epoch_id, reward));

        reward
    }

    pub fn get_position(env: Env, provider: Address) -> MiningPosition {
        get_position(&env, &provider)
    }

    fn require_no_pending_claims(env: &Env, provider: &Address) {
        let pos = get_position(env, provider);
        let current_id = get_current_epoch_id(env);
        
        // If there are closed epochs the user hasn't claimed yet, block modification
        for i in (pos.last_claim_epoch + 1)..=current_id {
            if let Some(epoch) = get_epoch(env, i) {
                if epoch.end_at != 0 && !has_claimed(env, provider, i) {
                    panic!("Must claim past epochs first");
                }
            }
        }
    }
}
