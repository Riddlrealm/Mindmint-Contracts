#![no_std]

mod storage;
#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, Address, Env, Vec, symbol_short};
use soroban_sdk::token::Client as TokenClient;
use crate::storage::*;

#[contract]
pub struct DelegationContract;

#[contractimpl]
impl DelegationContract {
    pub fn initialize(env: Env, token_address: Address) {
        if env.storage().instance().has(&DataKey::TokenAddress) {
            panic!("Already initialized");
        }
        set_token_address(&env, &token_address);
    }

    pub fn delegate(env: Env, delegator: Address, delegatee: Address) {
        delegator.require_auth();

        if delegator == delegatee {
            panic!("Cannot delegate to self");
        }

        // Circular delegation check
        let mut curr = delegatee.clone();
        for _ in 0..10 {
            if curr == delegator {
                panic!("Circular delegation detected");
            }
            if let Some(d) = get_delegation(&env, &curr) {
                if d.active {
                    curr = d.delegatee;
                    continue;
                }
            }
            break;
        }

        // Remove from old delegatee if exists
        if let Some(mut old_d) = get_delegation(&env, &delegator) {
            if old_d.active {
                remove_delegator(&env, &old_d.delegatee, &delegator);
            }
        }

        // Add to new delegatee
        add_delegator(&env, &delegatee, &delegator);

        let new_delegation = Delegation {
            delegator: delegator.clone(),
            delegatee: delegatee.clone(),
            delegated_at: env.ledger().timestamp(),
            active: true,
        };

        set_delegation(&env, &delegator, &new_delegation);

        env.events().publish((symbol_short!("Delegate"), symbol_short!("Set")), (delegator, delegatee));
    }

    pub fn revoke_delegation(env: Env, delegator: Address) {
        delegator.require_auth();

        if let Some(mut d) = get_delegation(&env, &delegator) {
            if d.active {
                d.active = false;
                remove_delegator(&env, &d.delegatee, &delegator);
                set_delegation(&env, &delegator, &d);
                env.events().publish((symbol_short!("Delegate"), symbol_short!("Revoked")), delegator);
            }
        }
    }

    pub fn get_delegator_chain(env: Env, start: Address) -> Vec<Address> {
         let mut chain = Vec::new(&env);
         let mut current = start.clone();
         chain.push_back(current.clone());
         
         for _ in 0..10 {
             if let Some(d) = get_delegation(&env, &current) {
                 if d.active {
                     current = d.delegatee;
                     chain.push_back(current.clone());
                     continue;
                 }
             }
             break;
         }
         chain
    }

    pub fn get_voting_power(env: Env, address: Address) -> i128 {
        let token = get_token_address(&env);
        let client = TokenClient::new(&env, &token);
        
        if let Some(d) = get_delegation(&env, &address) {
            if d.active {
                return 0; // Balance forwarded to delegatee
            }
        }
        
        let own_balance = client.balance(&address);
        own_balance + Self::compute_delegated_power(&env, &address, &client, 1, 3)
    }

    fn compute_delegated_power(env: &Env, delegatee: &Address, client: &TokenClient, depth: u32, max_depth: u32) -> i128 {
        if depth > max_depth {
            return 0;
        }
        
        let mut total = 0;
        let delegators = get_delegators(env, delegatee);
        
        for delegator in delegators.iter() {
            if let Some(d) = get_delegation(env, &delegator) {
                if d.active && d.delegatee == *delegatee {
                    let bal = client.balance(&delegator);
                    total += bal;
                    total += Self::compute_delegated_power(env, &delegator, client, depth + 1, max_depth);
                }
            }
        }
        total
    }
}
