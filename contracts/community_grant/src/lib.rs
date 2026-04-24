#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, String, Symbol, Vec,
};

const VOTING_WINDOW_SECONDS: u64 = 7 * 24 * 60 * 60;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalStatus {
    Voting,
    Approved,
    Rejected,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantMilestone {
    pub description: String,
    pub amount: i128,
    pub claimed: bool,
    pub verified: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantProposal {
    pub id: u64,
    pub applicant: Address,
    pub amount_requested: i128,
    pub milestones: Vec<GrantMilestone>,
    pub votes_for: i128,
    pub votes_against: i128,
    pub status: ProposalStatus,
    pub approved_at: Option<u64>,
    pub created_at: u64,
    pub voting_ends_at: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub enum DataKey {
    Admin,
    Token,
    Quorum,
    NextProposalId,
    Proposal(u64),
    HasVoted(u64, Address),
}

#[contract]
pub struct CommunityGrantContract;

#[contractimpl]
impl CommunityGrantContract {
    pub fn initialize(env: Env, admin: Address, token: Address, quorum: i128) {
        admin.require_auth();

        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        if quorum <= 0 {
            panic!("Invalid quorum");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::Quorum, &quorum);
        env.storage().instance().set(&DataKey::NextProposalId, &1u64);
    }

    pub fn submit_proposal(env: Env, applicant: Address, amount_requested: i128, milestones: Vec<GrantMilestone>) -> u64 {
        applicant.require_auth();

        if amount_requested <= 0 {
            panic!("Invalid amount requested");
        }
        if milestones.is_empty() {
            panic!("Milestones required");
        }

        let mut total_milestone_amount = 0i128;
        for milestone in milestones.iter() {
            if milestone.amount <= 0 {
                panic!("Milestone amount must be positive");
            }
            if milestone.claimed || milestone.verified {
                panic!("Invalid initial milestone state");
            }
            total_milestone_amount += milestone.amount;
        }

        if total_milestone_amount != amount_requested {
            panic!("Milestone total must equal amount requested");
        }

        let now = env.ledger().timestamp();
        let id = Self::next_proposal_id(&env);

        let proposal = GrantProposal {
            id,
            applicant: applicant.clone(),
            amount_requested,
            milestones,
            votes_for: 0,
            votes_against: 0,
            status: ProposalStatus::Voting,
            approved_at: None,
            created_at: now,
            voting_ends_at: now + VOTING_WINDOW_SECONDS,
        };

        env.storage().persistent().set(&DataKey::Proposal(id), &proposal);

        env.events().publish(
            (Symbol::new(&env, "ProposalSubmitted"), id),
            (applicant, amount_requested, proposal.voting_ends_at),
        );

        id
    }

    pub fn vote(env: Env, voter: Address, proposal_id: u64, support: bool) {
        voter.require_auth();

        let mut proposal = Self::get_proposal_or_panic(&env, proposal_id);
        Self::resolve_if_window_closed(&env, &mut proposal);

        if proposal.status != ProposalStatus::Voting {
            panic!("Proposal not in voting");
        }
        if env.ledger().timestamp() > proposal.voting_ends_at {
            panic!("Voting window closed");
        }

        let voted_key = DataKey::HasVoted(proposal_id, voter.clone());
        if env.storage().persistent().has(&voted_key) {
            panic!("Already voted");
        }

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let token_client = token::Client::new(&env, &token_addr);
        let weight = token_client.balance(&voter);
        if weight <= 0 {
            panic!("No voting power");
        }

        if support {
            proposal.votes_for += weight;
        } else {
            proposal.votes_against += weight;
        }

        env.storage().persistent().set(&voted_key, &true);
        env.storage().persistent().set(&DataKey::Proposal(proposal_id), &proposal);

        env.events().publish(
            (Symbol::new(&env, "VoteCast"), proposal_id),
            (voter, support, weight),
        );
    }

    pub fn verify_milestone(env: Env, admin: Address, proposal_id: u64, milestone_index: u32) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut proposal = Self::get_proposal_or_panic(&env, proposal_id);
        Self::resolve_if_window_closed(&env, &mut proposal);

        if proposal.status != ProposalStatus::Approved {
            panic!("Proposal not approved");
        }

        let mut milestone = proposal
            .milestones
            .get(milestone_index)
            .expect("Milestone not found");

        if milestone.verified {
            panic!("Milestone already verified");
        }

        milestone.verified = true;
        proposal.milestones.set(milestone_index, milestone);
        env.storage().persistent().set(&DataKey::Proposal(proposal_id), &proposal);

        env.events().publish(
            (Symbol::new(&env, "MilestoneVerified"), proposal_id),
            milestone_index,
        );
    }

    pub fn claim_milestone(env: Env, applicant: Address, proposal_id: u64, milestone_index: u32) {
        applicant.require_auth();

        let mut proposal = Self::get_proposal_or_panic(&env, proposal_id);
        Self::resolve_if_window_closed(&env, &mut proposal);

        if proposal.status != ProposalStatus::Approved {
            panic!("Proposal not approved");
        }
        if proposal.applicant != applicant {
            panic!("Only applicant can claim");
        }

        let mut milestone = proposal
            .milestones
            .get(milestone_index)
            .expect("Milestone not found");

        if !milestone.verified {
            panic!("Milestone not verified");
        }
        if milestone.claimed {
            panic!("Milestone already claimed");
        }

        milestone.claimed = true;
        proposal.milestones.set(milestone_index, milestone.clone());

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let token_client = token::Client::new(&env, &token_addr);
        token_client.transfer(&env.current_contract_address(), &applicant, &milestone.amount);

        env.storage().persistent().set(&DataKey::Proposal(proposal_id), &proposal);

        env.events().publish(
            (Symbol::new(&env, "MilestoneClaimed"), proposal_id),
            (milestone_index, applicant, milestone.amount),
        );
    }

    pub fn get_proposal(env: Env, proposal_id: u64) -> GrantProposal {
        let mut proposal = Self::get_proposal_or_panic(&env, proposal_id);
        Self::resolve_if_window_closed(&env, &mut proposal);
        proposal
    }

    fn get_proposal_or_panic(env: &Env, proposal_id: u64) -> GrantProposal {
        env.storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("Proposal not found")
    }

    fn resolve_if_window_closed(env: &Env, proposal: &mut GrantProposal) {
        if proposal.status != ProposalStatus::Voting {
            return;
        }
        if env.ledger().timestamp() <= proposal.voting_ends_at {
            return;
        }

        let quorum: i128 = env.storage().instance().get(&DataKey::Quorum).unwrap();
        let total_votes = proposal.votes_for + proposal.votes_against;
        let has_quorum = total_votes >= quorum;
        let has_majority = proposal.votes_for > proposal.votes_against;

        if has_quorum && has_majority {
            proposal.status = ProposalStatus::Approved;
            proposal.approved_at = Some(proposal.voting_ends_at);
            env.events().publish(
                (Symbol::new(env, "ProposalApproved"), proposal.id),
                (proposal.votes_for, proposal.votes_against, quorum),
            );
        } else {
            proposal.status = ProposalStatus::Rejected;
        }

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal.id), proposal);
    }

    fn next_proposal_id(env: &Env) -> u64 {
        let id: u64 = env.storage().instance().get(&DataKey::NextProposalId).unwrap();
        env.storage().instance().set(&DataKey::NextProposalId, &(id + 1));
        id
    }

    fn assert_admin(env: &Env, user: &Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != *user {
            panic!("Admin only");
        }
    }
}

#[cfg(test)]
mod test;
