use soroban_sdk::{Env, Address};
use crate::types::{DataKey, VotingConfig, PuzzleVote, PuzzleVotingAggregate};

//
// ──────────────────────────────────────────────────────────
// CONFIG STORAGE
// ──────────────────────────────────────────────────────────
//

pub fn set_config(env: &Env, config: &VotingConfig) {
    env.storage().instance().set(&DataKey::Config, config);
}

pub fn get_config(env: &Env) -> Option<VotingConfig> {
    env.storage().instance().get(&DataKey::Config)
}

pub fn has_config(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Config)
}

//
// ──────────────────────────────────────────────────────────
// VOTE STORAGE
// ──────────────────────────────────────────────────────────
//

/// Store a vote cast by a voter on a puzzle
pub fn set_vote(env: &Env, voter: &Address, puzzle_id: u32, vote: &PuzzleVote) {
    env.storage()
        .persistent()
        .set(&DataKey::Vote(voter.clone(), puzzle_id), vote);
}

/// Retrieve a vote cast by a specific voter on a specific puzzle
pub fn get_vote(env: &Env, voter: &Address, puzzle_id: u32) -> Option<PuzzleVote> {
    env.storage()
        .persistent()
        .get(&DataKey::Vote(voter.clone(), puzzle_id))
}

/// Check if a vote exists for a voter-puzzle pair
pub fn has_vote(env: &Env, voter: &Address, puzzle_id: u32) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Vote(voter.clone(), puzzle_id))
}

//
// ──────────────────────────────────────────────────────────
// AGGREGATE STORAGE
// ──────────────────────────────────────────────────────────
//

/// Store aggregated voting data for a puzzle
pub fn set_aggregate(env: &Env, puzzle_id: u32, aggregate: &PuzzleVotingAggregate) {
    env.storage()
        .persistent()
        .set(&DataKey::Aggregate(puzzle_id), aggregate);
}

/// Retrieve aggregated voting data for a puzzle
pub fn get_aggregate(env: &Env, puzzle_id: u32) -> Option<PuzzleVotingAggregate> {
    env.storage()
        .persistent()
        .get(&DataKey::Aggregate(puzzle_id))
}

//
// ──────────────────────────────────────────────────────────
// VOTE COUNT STORAGE
// ──────────────────────────────────────────────────────────
//

/// Get the total number of votes for a puzzle
pub fn get_vote_count(env: &Env, puzzle_id: u32) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::VoteCount(puzzle_id))
        .unwrap_or(0)
}

/// Increment the vote count for a puzzle
pub fn increment_vote_count(env: &Env, puzzle_id: u32) {
    let count = get_vote_count(env, puzzle_id);
    env.storage()
        .persistent()
        .set(&DataKey::VoteCount(puzzle_id), &(count + 1));
}

/// Reset the vote count for a puzzle
pub fn reset_vote_count(env: &Env, puzzle_id: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::VoteCount(puzzle_id), &0u32);
}
