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
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum ProposalCategory {
    PuzzleCuration = 0,
    Rewards = 1,
    PlatformRules = 2,
    Treasury = 3,
    Membership = 4,
    Emergency = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum MembershipTier {
    Basic = 0,
    Active = 1,
    Premium = 2,
    Council = 3,
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
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceConfig {
    pub voting_delay: u64,
    pub voting_period: u64,
    pub proposal_threshold: i128,
    pub quorum_percentage: u32,
    pub token_address: Address,
    pub treasury_address: Address,
    pub execution_delay: u64,
    pub emergency_quorum_percentage: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Member {
    pub address: Address,
    pub tier: MembershipTier,
    pub joined_at: u64,
    pub reputation: u64,
    pub is_active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreasuryInfo {
    pub total_balance: i128,
    pub allocated_funds: i128,
    pub last_distribution: u64,
}

// ── Multisig types (ADR-0013) ────────────────────────────────────────────

/// Configuration for the multisig gate (ADR-0013).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultisigConfig {
    /// Minimum number of Council-tier signatures required.
    pub threshold: u32,
    /// Maximum time (in seconds) a pending admin action may collect signatures.
    pub action_ttl: u64,
}

/// Status of a multisig-gated admin action.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdminActionStatus {
    /// Awaiting additional signatures.
    Pending,
    /// Threshold reached; ready to execute.
    Approved,
    /// Already executed.
    Executed,
    /// Rejected or expired.
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
    Member(Address),
    MemberCount,
    TreasuryInfo,
    MembershipThresholds,
    MultisigConfig,
    AdminAction(u64),
    AdminActionCount,
}
