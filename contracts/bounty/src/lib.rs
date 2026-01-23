#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, token,
};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BountyStatus {
    Open = 0,
    Accepted = 1,
    Submitted = 2,
    Completed = 3,
    Cancelled = 4,
    Disputed = 5,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Bounty {
    pub id: u32,
    pub creator: Address,
    pub token: Address,
    pub amount: i128,
    pub puzzle_id: Option<u32>,
    pub solver: Option<Address>,
    pub expiration: u64,
    pub status: BountyStatus,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Bounty(u32),
    BountyCount,
}

#[contract]
pub struct BountyContract;

#[contractimpl]
impl BountyContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::BountyCount, &0u32);
    }

    pub fn create_bounty(
        env: Env,
        creator: Address,
        token_address: Address,
        amount: i128,
        puzzle_id: Option<u32>,
        duration: u64,
    ) -> u32 {
        creator.require_auth();

        if amount <= 0 {
            panic!("Amount must be positive");
        }

        let mut count: u32 = env.storage().instance().get(&DataKey::BountyCount).unwrap_or(0);
        count += 1;

        let expiration = env.ledger().timestamp() + duration;

        let bounty = Bounty {
            id: count,
            creator: creator.clone(),
            token: token_address.clone(),
            amount,
            puzzle_id,
            solver: None,
            expiration,
            status: BountyStatus::Open,
        };

        // Escrow funds: transfer from creator to this contract
        let token_client = token::Client::new(&env, &token_address);
        token_client.transfer(&creator, &env.current_contract_address(), &amount);

        env.storage().instance().set(&DataKey::Bounty(count), &bounty);
        env.storage().instance().set(&DataKey::BountyCount, &count);

        env.events().publish(
            (symbol_short!("bounty"), symbol_short!("created")),
            (count, creator, amount),
        );

        count
    }

    pub fn accept_bounty(env: Env, solver: Address, bounty_id: u32) {
        solver.require_auth();

        let mut bounty = self::BountyContract::get_bounty(env.clone(), bounty_id).expect("Bounty not found");

        if bounty.status != BountyStatus::Open {
            panic!("Bounty is not open");
        }

        if env.ledger().timestamp() > bounty.expiration {
            panic!("Bounty has expired");
        }

        bounty.solver = Some(solver.clone());
        bounty.status = BountyStatus::Accepted;

        env.storage().instance().set(&DataKey::Bounty(bounty_id), &bounty);

        env.events().publish(
            (symbol_short!("bounty"), symbol_short!("accepted")),
            (bounty_id, solver),
        );
    }

    pub fn submit_solution(env: Env, solver: Address, bounty_id: u32) {
        solver.require_auth();

        let mut bounty = self::BountyContract::get_bounty(env.clone(), bounty_id).expect("Bounty not found");

        if bounty.status != BountyStatus::Accepted {
            panic!("Bounty not accepted");
        }

        if Some(solver.clone()) != bounty.solver {
            panic!("Not the assigned solver");
        }

        if env.ledger().timestamp() > bounty.expiration {
            panic!("Bounty has expired");
        }

        bounty.status = BountyStatus::Submitted;

        env.storage().instance().set(&DataKey::Bounty(bounty_id), &bounty);

        env.events().publish(
            (symbol_short!("bounty"), symbol_short!("submitted")),
            (bounty_id, solver),
        );
    }

    pub fn approve_submission(env: Env, creator: Address, bounty_id: u32) {
        creator.require_auth();

        let mut bounty = self::BountyContract::get_bounty(env.clone(), bounty_id).expect("Bounty not found");

        if bounty.creator != creator {
            panic!("Not the creator");
        }

        if bounty.status != BountyStatus::Submitted {
            panic!("No submission to approve");
        }

        let solver = bounty.solver.clone().expect("No solver found");

        // Release funds to solver
        let token_client = token::Client::new(&env, &bounty.token);
        token_client.transfer(&env.current_contract_address(), &solver, &bounty.amount);

        bounty.status = BountyStatus::Completed;

        env.storage().instance().set(&DataKey::Bounty(bounty_id), &bounty);

        env.events().publish(
            (symbol_short!("bounty"), symbol_short!("completed")),
            (bounty_id, creator, solver),
        );
    }

    pub fn cancel_bounty(env: Env, creator: Address, bounty_id: u32) {
        creator.require_auth();

        let mut bounty = self::BountyContract::get_bounty(env.clone(), bounty_id).expect("Bounty not found");

        if bounty.creator != creator {
            panic!("Not the creator");
        }

        let can_cancel = match bounty.status {
            BountyStatus::Open => true,
            BountyStatus::Accepted | BountyStatus::Submitted => env.ledger().timestamp() > bounty.expiration,
            _ => false,
        };

        if !can_cancel {
            panic!("Cannot cancel at this stage or not yet expired");
        }

        // Refund creator
        let token_client = token::Client::new(&env, &bounty.token);
        token_client.transfer(&env.current_contract_address(), &creator, &bounty.amount);

        bounty.status = BountyStatus::Cancelled;

        env.storage().instance().set(&DataKey::Bounty(bounty_id), &bounty);

        env.events().publish(
            (symbol_short!("bounty"), symbol_short!("cancelled")),
            (bounty_id, creator),
        );
    }

    pub fn dispute_bounty(env: Env, caller: Address, bounty_id: u32) {
        caller.require_auth();

        let mut bounty = self::BountyContract::get_bounty(env.clone(), bounty_id).expect("Bounty not found");

        if caller != bounty.creator && Some(caller.clone()) != bounty.solver {
            panic!("Only creator or solver can dispute");
        }

        if bounty.status != BountyStatus::Submitted && bounty.status != BountyStatus::Accepted {
            panic!("Cannot dispute at this stage");
        }

        bounty.status = BountyStatus::Disputed;

        env.storage().instance().set(&DataKey::Bounty(bounty_id), &bounty);

        env.events().publish(
            (symbol_short!("bounty"), symbol_short!("disputed")),
            (bounty_id, caller),
        );
    }

    pub fn resolve_dispute(env: Env, admin: Address, bounty_id: u32, solver_payout: i128) {
        admin.require_auth();

        let admin_stored: Address = env.storage().instance().get(&DataKey::Admin).expect("No admin set");
        if admin != admin_stored {
            panic!("Only admin can resolve disputes");
        }

        let mut bounty = self::BountyContract::get_bounty(env.clone(), bounty_id).expect("Bounty not found");

        if bounty.status != BountyStatus::Disputed {
            panic!("Bounty is not in dispute");
        }

        if solver_payout < 0 || solver_payout > bounty.amount {
            panic!("Invalid payout amount");
        }

        let creator_payout = bounty.amount - solver_payout;
        let token_client = token::Client::new(&env, &bounty.token);

        if solver_payout > 0 {
            let solver = bounty.solver.clone().expect("No solver to pay");
            token_client.transfer(&env.current_contract_address(), &solver, &solver_payout);
        }

        if creator_payout > 0 {
            token_client.transfer(&env.current_contract_address(), &bounty.creator, &creator_payout);
        }

        bounty.status = BountyStatus::Completed; // Or create a generic 'Resolved' status

        env.storage().instance().set(&DataKey::Bounty(bounty_id), &bounty);

        env.events().publish(
            (symbol_short!("bounty"), symbol_short!("resolved")),
            (bounty_id, solver_payout),
        );
    }

    pub fn get_active_bounties(env: Env, offset: u32, limit: u32) -> soroban_sdk::Vec<Bounty> {
        let count = Self::get_bounty_count(env.clone());
        let mut bounties = soroban_sdk::Vec::new(&env);
        
        if offset > count {
            return bounties;
        }

        let end = (offset + limit).min(count + 1);
        for i in (offset + 1)..end {
            if let Some(bounty) = Self::get_bounty(env.clone(), i) {
                if bounty.status == BountyStatus::Open || bounty.status == BountyStatus::Accepted || bounty.status == BountyStatus::Submitted {
                    bounties.push_back(bounty);
                }
            }
        }
        bounties
    }

    pub fn get_bounty(env: Env, bounty_id: u32) -> Option<Bounty> {
        env.storage().instance().get(&DataKey::Bounty(bounty_id))
    }

    pub fn get_bounty_count(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::BountyCount).unwrap_or(0)
    }
}

mod test;
