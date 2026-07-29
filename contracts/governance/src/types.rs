use soroban_sdk::{contracttype, Address, String, Symbol, Val, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalStatus {
    Pending,
    Active,
    Defeated,
    Succeeded,
    Executed,
    Canceled,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoteType {
    For,
    Against,
    Abstain,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalAction {
    pub contract_id: Address,
    pub function_name: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalActionInput {
    pub contract_id: Address,
    pub function_name: Symbol,
    pub args: Vec<Val>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u64,
    pub proposer: Address,
    pub title: String,
    pub description: String,
    pub action: ProposalAction,
    pub start_time: u64,
    pub end_time: u64,
    pub for_votes: i128,
    pub against_votes: i128,
    pub abstain_votes: i128,
    pub status: ProposalStatus,
    pub quorum: i128,
    pub category: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceConfig {
    pub voting_delay: u64,
    pub voting_period: u64,
    pub proposal_threshold: i128,
    pub quorum_percentage: u32,
    pub token_address: Address,
}

// ───────────── MULTISIG TYPES (ADR-0013) ─────────────

/// Threshold configuration for multisig governance.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultisigConfig {
    /// Number of unique Council signer approvals required.
    pub threshold: u32,
    /// Seconds before an admin action expires if not fully approved/executed.
    pub action_ttl: u64,
}

/// Execution status of a multisig-gated admin action.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdminActionStatus {
    Pending,
    Approved,
    Executed,
    Rejected,
}

/// A multisig-gated admin action (treasury allocation, threshold update, etc.).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminAction {
    pub id: u64,
    pub proposer: Address,
    pub description: String,
    pub status: AdminActionStatus,
    pub created_at: u64,
    pub expires_at: u64,
    pub executed_at: Option<u64>,
    /// Council members who have signed (includes proposer).
    pub signers: Vec<Address>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Config,
    Proposal(u64),
    ProposalCount,
    TokenBalance(Address),
    VotingPower(Address),
    Delegation(Address),
    Vote(u64, Address),
    ProposalArgs(u64),
    // Multisig keys
    MultisigConfig,
    AdminAction(u64),
    AdminActionCount,
}
