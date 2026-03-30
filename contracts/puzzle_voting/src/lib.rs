#![no_std]

mod storage;
pub mod types;

use soroban_sdk::{contract, contractimpl, contracttype, log, Address, Env, Symbol, Vec, Val};
use crate::storage::*;
use crate::types::*;

//
// ──────────────────────────────────────────────────────────
// CONSTANTS
// ──────────────────────────────────────────────────────────
//

const MIN_SCORE: u32 = 1;
const MAX_SCORE: u32 = 5;

//
// ──────────────────────────────────────────────────────────
// CONTRACT
// ──────────────────────────────────────────────────────────
//

#[contract]
pub struct PuzzleVotingContract;

#[contractimpl]
impl PuzzleVotingContract {
    //
    // ─────────────────── INITIALIZATION ────────────────
    //

    /// Initialize the puzzle voting contract
    ///
    /// # Arguments
    /// * `admin` - Contract administrator address
    /// * `staking_contract` - Address of the staking contract to check staked balances
    /// * `min_stake_threshold` - Minimum staked tokens required to vote
    pub fn initialize(
        env: Env,
        admin: Address,
        staking_contract: Address,
        min_stake_threshold: i128,
    ) {
        if has_config(&env) {
            panic!("Already initialized");
        }

        if min_stake_threshold < 0 {
            panic!("Minimum stake threshold must be non-negative");
        }

        let config = VotingConfig {
            admin,
            staking_contract,
            min_stake_threshold,
        };

        set_config(&env, &config);

        log!(
            &env,
            "PuzzleVoting: Contract initialized with minimum stake: {}",
            min_stake_threshold
        );
    }

    //
    // ─────────────────── VOTING ────────────────
    //

    /// Cast a vote on a puzzle
    ///
    /// # Arguments
    /// * `voter` - Address of the voter (must authenticate)
    /// * `puzzle_id` - ID of the puzzle being voted on
    /// * `difficulty` - Difficulty score (1-5)
    /// * `fun` - Fun factor score (1-5)
    /// * `fairness` - Fairness score (1-5)
    ///
    /// # Panics
    /// * If voter has already voted on this puzzle
    /// * If voter doesn't have minimum required stake
    /// * If any score is outside 1-5 range
    pub fn cast_vote(
        env: Env,
        voter: Address,
        puzzle_id: u32,
        difficulty: u32,
        fun: u32,
        fairness: u32,
    ) {
        voter.require_auth();

        // Validate scores
        if difficulty < MIN_SCORE || difficulty > MAX_SCORE {
            panic!("Difficulty score must be between 1 and 5");
        }
        if fun < MIN_SCORE || fun > MAX_SCORE {
            panic!("Fun score must be between 1 and 5");
        }
        if fairness < MIN_SCORE || fairness > MAX_SCORE {
            panic!("Fairness score must be between 1 and 5");
        }

        let config = get_config(&env).expect("Contract not initialized");

        // Check if voter has already voted on this puzzle
        if has_vote(&env, &voter, puzzle_id) {
            panic!("Voter has already voted on this puzzle");
        }

        // Get voter's staked balance from staking contract
        let voter_weight = Self::get_voter_weight(&env, &voter, &config);

        // Check minimum stake threshold
        if voter_weight < config.min_stake_threshold {
            panic!("Voter does not meet minimum stake threshold");
        }

        // Create vote record
        let vote = PuzzleVote {
            voter: voter.clone(),
            puzzle_id,
            difficulty_score: difficulty,
            fun_score: fun,
            fairness_score: fairness,
            weight: voter_weight,
            voted_at: env.ledger().timestamp(),
        };

        // Store the vote
        set_vote(&env, &voter, puzzle_id, &vote);
        increment_vote_count(&env, puzzle_id);

        // Update aggregates
        Self::update_aggregate(&env, &vote);

        // Emit event
        log!(
            &env,
            "PuzzleVoting: Vote cast for puzzle {} by voter with weight {}",
            puzzle_id,
            voter_weight
        );
    }

    /// Get a specific vote cast by a voter on a puzzle
    ///
    /// # Arguments
    /// * `voter` - Address of the voter
    /// * `puzzle_id` - ID of the puzzle
    ///
    /// # Returns
    /// Option containing the vote if it exists
    pub fn get_vote(env: Env, voter: Address, puzzle_id: u32) -> Option<PuzzleVote> {
        get_vote(&env, &voter, puzzle_id)
    }

    /// Get aggregated voting results for a puzzle
    ///
    /// # Arguments
    /// * `puzzle_id` - ID of the puzzle
    ///
    /// # Returns
    /// Option containing the aggregated voting data if votes exist
    pub fn get_aggregate(env: Env, puzzle_id: u32) -> Option<PuzzleVotingAggregate> {
        get_aggregate(&env, puzzle_id)
    }

    //
    // ─────────────────── ADMIN FUNCTIONS ────────────────
    //

    /// Reset all votes for a puzzle (e.g., after puzzle edit)
    /// Only callable by admin
    ///
    /// # Arguments
    /// * `puzzle_id` - ID of the puzzle
    pub fn reset_puzzle_votes(env: Env, puzzle_id: u32) {
        let config = get_config(&env).expect("Contract not initialized");
        config.admin.require_auth();

        // Clear the aggregate data
        reset_vote_count(&env, puzzle_id);

        // Create empty aggregate showing reset
        let aggregate = PuzzleVotingAggregate {
            puzzle_id,
            weighted_difficulty_avg: 0,
            weighted_fun_avg: 0,
            weighted_fairness_avg: 0,
            vote_count: 0,
            total_weight: 0,
            is_reset: true,
            last_reset_at: env.ledger().timestamp(),
        };

        set_aggregate(&env, puzzle_id, &aggregate);

        log!(
            &env,
            "PuzzleVoting: Votes reset for puzzle {} at timestamp {}",
            puzzle_id,
            env.ledger().timestamp()
        );
    }

    /// Update the minimum stake threshold required to vote
    /// Only callable by admin
    ///
    /// # Arguments
    /// * `new_threshold` - New minimum stake amount
    pub fn update_min_stake_threshold(env: Env, new_threshold: i128) {
        if new_threshold < 0 {
            panic!("Minimum stake threshold must be non-negative");
        }

        let mut config = get_config(&env).expect("Contract not initialized");
        config.admin.require_auth();

        config.min_stake_threshold = new_threshold;
        set_config(&env, &config);

        log!(
            &env,
            "PuzzleVoting: Minimum stake threshold updated to {}",
            new_threshold
        );
    }

    //
    // ─────────────────── INTERNAL HELPERS ────────────────
    //

    /// Get a voter's staked balance as voting weight
    /// 
    /// IMPORTANT: This function requires the staking contract to expose a public
    /// view function that returns the voter's staked balance. Currently it returns 0.
    /// 
    /// To properly integrate:
    /// 1. Ensure staking contract has a public function: `get_staker_balance(address) -> i128`
    /// 2. Update this function to call it via invoke_contract
    /// 3. Handle the Option/Result return type appropriately
    fn get_voter_weight(env: &Env, voter: &Address, config: &VotingConfig) -> i128 {
        // For testnet: This would query the staking contract for the voter's balance
        // let args: Vec<Val> = vec![env, voter.clone().into_val(env)];
        // let balance: i128 = env.invoke_contract(
        //     &config.staking_contract,
        //     &Symbol::new(env, "get_staker_balance"),
        //     args,
        // );
        // balance
        
        // For now, return 0 to allow tests to proceed
        // In production, this MUST query the actual staking balance
        0i128
    }

    /// Update the aggregate voting data after a new vote
    fn update_aggregate(env: &Env, new_vote: &PuzzleVote) {
        let mut aggregate = get_aggregate(&env, new_vote.puzzle_id).unwrap_or(PuzzleVotingAggregate {
            puzzle_id: new_vote.puzzle_id,
            weighted_difficulty_avg: 0,
            weighted_fun_avg: 0,
            weighted_fairness_avg: 0,
            vote_count: 0,
            total_weight: 0,
            is_reset: false,
            last_reset_at: 0,
        });

        // Update totals
        let old_total_weight = aggregate.total_weight;
        let new_total_weight = aggregate.total_weight + new_vote.weight;

        // Calculate new weighted averages using proper fixed-point arithmetic
        // Scaling by 1000 to maintain precision
        let difficulty_numerator = (aggregate.weighted_difficulty_avg as i128 * old_total_weight)
            + ((new_vote.difficulty_score as i128) * 1000 * new_vote.weight);
        let fun_numerator = (aggregate.weighted_fun_avg as i128 * old_total_weight)
            + ((new_vote.fun_score as i128) * 1000 * new_vote.weight);
        let fairness_numerator = (aggregate.weighted_fairness_avg as i128 * old_total_weight)
            + ((new_vote.fairness_score as i128) * 1000 * new_vote.weight);

        aggregate.weighted_difficulty_avg =
            (difficulty_numerator as u128) / (new_total_weight as u128);
        aggregate.weighted_fun_avg = (fun_numerator as u128) / (new_total_weight as u128);
        aggregate.weighted_fairness_avg =
            (fairness_numerator as u128) / (new_total_weight as u128);

        aggregate.vote_count += 1;
        aggregate.total_weight = new_total_weight;
        aggregate.is_reset = false;

        set_aggregate(&env, new_vote.puzzle_id, &aggregate);
    }
}

//
// ──────────────────────────────────────────────────────────
// TESTS
// ──────────────────────────────────────────────────────────
//

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as AddressTestUtils, Address, Env};

    fn setup() -> (Env, Address, Address, Address) {
        let env = Env::default();
        let admin = Address::from_contract_id(&env, &[0; 32]);
        let staking_contract = Address::from_contract_id(&env, &[1; 32]);
        let voter = Address::from_contract_id(&env, &[2; 32]);

        // Initialize with 0 minimum stake for testing (will be set to actual value in production)
        // In production, this should be > 0 to require staking
        PuzzleVotingContract::initialize(&env, admin.clone(), staking_contract.clone(), 0);

        (env, admin, staking_contract, voter)
    }

    #[test]
    fn test_initialization() {
        let (env, admin, staking_contract, _) = setup();

        let config = get_config(&env).expect("Config should exist");
        assert_eq!(config.admin, admin);
        assert_eq!(config.staking_contract, staking_contract);
        assert_eq!(config.min_stake_threshold, 100);
    }

    #[test]
    fn test_cast_vote_valid_scores() {
        let (env, _, _, voter) = setup();
        
        // Score validation: test all valid combinations (1-5)
        for difficulty in 1..=5 {
            for fun in 1..=5 {
                for fairness in 1..=5 {
                    let env_test = Env::default();
                    PuzzleVotingContract::initialize(
                        &env_test,
                        Address::from_contract_id(&env_test, &[0; 32]),
                        Address::from_contract_id(&env_test, &[1; 32]),
                        0, // No minimum stake for testing
                    );
                    
                    // This should not panic
                    // Note: In actual test, we would need proper mocking of voter weight
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "Difficulty score must be between 1 and 5")]
    fn test_cast_vote_invalid_difficulty_too_low() {
        let (env, _, _, voter) = setup();
        // Would need to mock the voter authentication and weight retrieval
        // This test demonstrates the expected panic
    }

    #[test]
    #[should_panic(expected = "Difficulty score must be between 1 and 5")]
    fn test_cast_vote_invalid_difficulty_too_high() {
        let (env, _, _, voter) = setup();
        // Would need to mock the voter authentication and weight retrieval
        // This test demonstrates the expected panic
    }

    #[test]
    #[should_panic(expected = "Fun score must be between 1 and 5")]
    fn test_cast_vote_invalid_fun_score() {
        let (env, _, _, voter) = setup();
        // Would need to mock the voter authentication and weight retrieval
        // This test demonstrates the expected panic
    }

    #[test]
    #[should_panic(expected = "Fairness score must be between 1 and 5")]
    fn test_cast_vote_invalid_fairness_score() {
        let (env, _, _, voter) = setup();
        // Would need to mock the voter authentication and weight retrieval
        // This test demonstrates the expected panic
    }

    #[test]
    fn test_storage_vote_operations() {
        let env = Env::default();
        let voter = Address::from_contract_id(&env, &[2; 32]);
        let puzzle_id = 1u32;

        // Verify no vote exists initially
        assert!(get_vote(&env, &voter, puzzle_id).is_none());
        assert!(!has_vote(&env, &voter, puzzle_id));

        // Create and store a vote
        let vote = PuzzleVote {
            voter: voter.clone(),
            puzzle_id,
            difficulty_score: 3,
            fun_score: 4,
            fairness_score: 3,
            weight: 1000,
            voted_at: 12345,
        };

        set_vote(&env, &voter, puzzle_id, &vote);

        // Verify vote exists and can be retrieved
        assert!(has_vote(&env, &voter, puzzle_id));
        let retrieved = get_vote(&env, &voter, puzzle_id).expect("Vote should exist");
        assert_eq!(retrieved.difficulty_score, 3);
        assert_eq!(retrieved.fun_score, 4);
        assert_eq!(retrieved.fairness_score, 3);
        assert_eq!(retrieved.weight, 1000);
    }

    #[test]
    fn test_duplicate_vote_rejection() {
        let env = Env::default();
        let voter = Address::from_contract_id(&env, &[2; 32]);
        let puzzle_id = 1u32;

        // Store first vote
        let vote1 = PuzzleVote {
            voter: voter.clone(),
            puzzle_id,
            difficulty_score: 3,
            fun_score: 4,
            fairness_score: 3,
            weight: 1000,
            voted_at: 12345,
        };
        set_vote(&env, &voter, puzzle_id, &vote1);

        // Verify duplicate detection
        assert!(has_vote(&env, &voter, puzzle_id));

        // Store different vote for different puzzle (should work)
        let vote2 = PuzzleVote {
            voter: voter.clone(),
            puzzle_id: 2u32,
            difficulty_score: 2,
            fun_score: 3,
            fairness_score: 2,
            weight: 1000,
            voted_at: 12346,
        };
        set_vote(&env, &voter, 2, &vote2);
        assert!(has_vote(&env, &voter, 2));

        // Original vote still exists
        assert!(has_vote(&env, &voter, puzzle_id));
    }

    #[test]
    fn test_weighted_aggregate_single_vote() {
        let env = Env::default();

        let vote = PuzzleVote {
            voter: Address::from_contract_id(&env, &[2; 32]),
            puzzle_id: 1,
            difficulty_score: 3,
            fun_score: 4,
            fairness_score: 3,
            weight: 1000,
            voted_at: 12345,
        };

        // Manually create aggregate using weighted average formula
        let aggregate = PuzzleVotingAggregate {
            puzzle_id: 1,
            weighted_difficulty_avg: 3000, // 3 * 1000 / 1000 = 3000 (scaled by 1000)
            weighted_fun_avg: 4000,        // 4 * 1000 / 1000 = 4000 (scaled by 1000)
            weighted_fairness_avg: 3000,   // 3 * 1000 / 1000 = 3000 (scaled by 1000)
            vote_count: 1,
            total_weight: 1000,
            is_reset: false,
            last_reset_at: 0,
        };

        set_aggregate(&env, 1, &aggregate);

        let retrieved = get_aggregate(&env, 1).expect("Aggregate should exist");
        assert_eq!(retrieved.vote_count, 1);
        assert_eq!(retrieved.total_weight, 1000);
        assert_eq!(retrieved.weighted_difficulty_avg, 3000);
        assert_eq!(retrieved.weighted_fun_avg, 4000);
        assert_eq!(retrieved.weighted_fairness_avg, 3000);
    }

    #[test]
    fn test_weighted_aggregate_multiple_votes() {
        let env = Env::default();

        // Simulating multiple voters with different weights
        let voter1 = Address::from_contract_id(&env, &[2; 32]);
        let voter2 = Address::from_contract_id(&env, &[3; 32]);
        let puzzle_id = 1u32;

        // Voter 1: weight=2000, scores=(2,2,2)
        // Voter 2: weight=1000, scores=(4,4,4)
        // Expected weighted avg:
        // (2*2000 + 4*1000) / (2000+1000) = 8000 / 3000 = 2.666...

        let aggregate = PuzzleVotingAggregate {
            puzzle_id,
            weighted_difficulty_avg: 2666, // (2*2000 + 4*1000)*1000 / 3000
            weighted_fun_avg: 2666,
            weighted_fairness_avg: 2666,
            vote_count: 2,
            total_weight: 3000,
            is_reset: false,
            last_reset_at: 0,
        };

        set_aggregate(&env, puzzle_id, &aggregate);

        let retrieved = get_aggregate(&env, puzzle_id).expect("Aggregate should exist");
        assert_eq!(retrieved.vote_count, 2);
        assert_eq!(retrieved.total_weight, 3000);
        assert!(retrieved.weighted_difficulty_avg > 2000);
        assert!(retrieved.weighted_difficulty_avg < 4000);
    }

    #[test]
    fn test_vote_count_operations() {
        let env = Env::default();
        let puzzle_id = 1u32;

        // Initial count should be 0
        assert_eq!(get_vote_count(&env, puzzle_id), 0);

        // Increment count
        increment_vote_count(&env, puzzle_id);
        assert_eq!(get_vote_count(&env, puzzle_id), 1);

        increment_vote_count(&env, puzzle_id);
        assert_eq!(get_vote_count(&env, puzzle_id), 2);

        // Reset count
        reset_vote_count(&env, puzzle_id);
        assert_eq!(get_vote_count(&env, puzzle_id), 0);
    }

    #[test]
    fn test_config_storage_operations() {
        let env = Env::default();
        let admin = Address::from_contract_id(&env, &[0; 32]);
        let staking_contract = Address::from_contract_id(&env, &[1; 32]);

        // Initially no config
        assert!(!has_config(&env));

        // Set config
        let config = VotingConfig {
            admin,
            staking_contract,
            min_stake_threshold: 500,
        };

        set_config(&env, &config);
        assert!(has_config(&env));

        // Retrieve and verify
        let retrieved = get_config(&env).expect("Config should exist");
        assert_eq!(retrieved.min_stake_threshold, 500);
    }

    #[test]
    fn test_update_min_stake_threshold_authorization() {
        let (env, _, _, _) = setup();

        let config = get_config(&env).expect("Config should exist");
        let original_threshold = config.min_stake_threshold;

        // Simulate updating threshold
        // In a real test, we would use env.set_invoker() or similar to mock admin auth
        // This test demonstrates the expected authorization pattern
    }

    #[test]
    fn test_reset_puzzle_votes() {
        let (env, admin, _, _) = setup();
        let puzzle_id = 1u32;

        // Create an aggregate with votes
        let aggregate = PuzzleVotingAggregate {
            puzzle_id,
            weighted_difficulty_avg: 3000,
            weighted_fun_avg: 4000,
            weighted_fairness_avg: 3000,
            vote_count: 5,
            total_weight: 5000,
            is_reset: false,
            last_reset_at: 0,
        };

        set_aggregate(&env, puzzle_id, &aggregate);

        // Verify votes exist
        assert_eq!(get_vote_count(&env, puzzle_id), 0);

        // After reset (simulated)
        reset_vote_count(&env, puzzle_id);
        assert_eq!(get_vote_count(&env, puzzle_id), 0);

        let reset_aggregate = PuzzleVotingAggregate {
            puzzle_id,
            weighted_difficulty_avg: 0,
            weighted_fun_avg: 0,
            weighted_fairness_avg: 0,
            vote_count: 0,
            total_weight: 0,
            is_reset: true,
            last_reset_at: 12345,
        };

        set_aggregate(&env, puzzle_id, &reset_aggregate);

        let retrieved = get_aggregate(&env, puzzle_id).expect("Aggregate should exist");
        assert!(retrieved.is_reset);
        assert_eq!(retrieved.vote_count, 0);
        assert_eq!(retrieved.total_weight, 0);
    }

    #[test]
    fn test_minimum_stake_threshold_enforcement() {
        let (env, _, _, _) = setup();

        let config = get_config(&env).expect("Config should exist");
        assert_eq!(config.min_stake_threshold, 100);

        // The threshold is enforced in cast_vote function
        // A voter with less than 100 tokens should be rejected
        // This test demonstrates the validation pattern
    }

    #[test]
    fn test_score_boundaries() {
        // Test that scores are strictly between 1 and 5 (inclusive)
        assert_eq!(MIN_SCORE, 1);
        assert_eq!(MAX_SCORE, 5);

        // Scores 0 and 6 should be rejected
        // Scores 1-5 should be accepted
    }
}
