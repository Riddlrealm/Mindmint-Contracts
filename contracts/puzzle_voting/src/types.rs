use soroban_sdk::{contracttype, Address};

/// Represents a single vote cast by a voter on a puzzle
#[contracttype]
#[derive(Clone, Debug)]
pub struct PuzzleVote {
    /// Address of the voter
    pub voter: Address,
    /// Puzzle ID being voted on
    pub puzzle_id: u32,
    /// Difficulty score 1-5
    pub difficulty_score: u32,
    /// Fun factor score 1-5
    pub fun_score: u32,
    /// Fairness score 1-5
    pub fairness_score: u32,
    /// Voting weight based on staked token balance at vote time
    pub weight: i128,
    /// Timestamp when the vote was cast
    pub voted_at: u64,
}

/// Aggregated voting results for a puzzle
#[contracttype]
#[derive(Clone, Debug)]
pub struct PuzzleVotingAggregate {
    /// Puzzle ID
    pub puzzle_id: u32,
    /// Weighted average difficulty score
    pub weighted_difficulty_avg: u128,
    /// Weighted average fun score
    pub weighted_fun_avg: u128,
    /// Weighted average fairness score
    pub weighted_fairness_avg: u128,
    /// Total vote count
    pub vote_count: u32,
    /// Total voting weight (sum of all weights)
    pub total_weight: i128,
    /// Flag indicating if votes have been reset (e.g., after puzzle edit)
    pub is_reset: bool,
    /// Timestamp of last reset
    pub last_reset_at: u64,
}

/// Configuration for the puzzle voting contract
#[contracttype]
#[derive(Clone, Debug)]
pub struct VotingConfig {
    /// Contract administrator
    pub admin: Address,
    /// Staking contract address (to check voter's staked balance)
    pub staking_contract: Address,
    /// Minimum staked balance required to vote
    pub min_stake_threshold: i128,
}

/// Events emitted by the contract
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VotingEvent {
    /// Emitted when a vote is cast
    VoteCast {
        voter: Address,
        puzzle_id: u32,
        difficulty_score: u32,
        fun_score: u32,
        fairness_score: u32,
        weight: i128,
    },
    /// Emitted when votes are reset for a puzzle
    VotesReset {
        puzzle_id: u32,
        reset_at: u64,
    },
    /// Emitted when minimum stake threshold is updated
    MinStakeThresholdUpdated {
        new_threshold: i128,
        updated_at: u64,
    },
}

/// Storage keys for data access
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// VotingConfig - stored in instance storage
    Config,
    /// PuzzleVote by (voter, puzzle_id)
    Vote(Address, u32),
    /// PuzzleVotingAggregate by puzzle_id
    Aggregate(u32),
    /// Total vote count per puzzle
    VoteCount(u32),
}
