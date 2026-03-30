# Puzzle Voting Contract

## Overview

The Puzzle Voting Contract enables players to cast on-chain votes rating individual puzzles across multiple dimensions with **token-weighted voting power**. Players with more staked tokens carry more voting weight in the collective scoring system.

### Key Features

- **Multi-dimensional voting** with difficulty, fun factor, and fairness ratings
- **Token-weighted voting** where voter weight = staked token balance
- **One vote per (voter, puzzle_id)** - duplicate votes rejected with clear error messages
- **Weighted aggregation** computing accurate weighted averages for each dimension
- **Vote reset capability** for admin to clear votes (e.g., after puzzle edits)
- **Configurable minimum stake** threshold to prevent spam voting
- **On-chain events** for vote casting and vote resets to enable off-chain indexing
- **Persistent on-chain storage** of all votes and aggregates for queryability

## Architecture

### Data Structures

#### PuzzleVote
```rust
pub struct PuzzleVote {
    pub voter: Address,              // Who cast the vote
    pub puzzle_id: u32,              // Which puzzle
    pub difficulty_score: u32,       // 1-5 scale
    pub fun_score: u32,              // 1-5 scale
    pub fairness_score: u32,         // 1-5 scale
    pub weight: i128,                // Staked balance = voting power
    pub voted_at: u64,               // Timestamp when voted
}
```

#### PuzzleVotingAggregate
```rust
pub struct PuzzleVotingAggregate {
    pub puzzle_id: u32,
    pub weighted_difficulty_avg: u128,   // Weighted average (scaled by 1000)
    pub weighted_fun_avg: u128,
    pub weighted_fairness_avg: u128,
    pub vote_count: u32,                 // Total votes
    pub total_weight: i128,              // Sum of all voter weights
    pub is_reset: bool,                  // Was aggregate reset?
    pub last_reset_at: u64,              // Timestamp of last reset
}
```

#### VotingConfig
```rust
pub struct VotingConfig {
    pub admin: Address,          // Can reset votes, update thresholds
    pub staking_contract: Address, // External contract providing voter weights
    pub min_stake_threshold: i128, // Minimum tokens needed to vote
}
```

### Storage Organization

- **Instance Storage**: Contract configuration (admin, staking contract, threshold)
- **Persistent Storage**: 
  - Individual votes indexed by (voter, puzzle_id)
  - Aggregated results indexed by puzzle_id
  - Vote counts per puzzle

## API Reference

### Initialization

```rust
fn initialize(
    env: Env,
    admin: Address,
    staking_contract: Address,
    min_stake_threshold: i128,
)
```

Initializes the contract with required configuration. Can only be called once.

**Parameters:**
- `admin`: Address that can reset votes and update thresholds
- `staking_contract`: Contract address providing voter stake information
- `min_stake_threshold`: Minimum staked tokens required to vote (e.g., 100)

### Voting Functions

#### cast_vote
```rust
fn cast_vote(
    env: Env,
    voter: Address,
    puzzle_id: u32,
    difficulty: u32,
    fun: u32,
    fairness: u32,
)
```

Cast a vote on a puzzle. Requires voter authentication.

**Duration:** Updates made immediately
**Cost:** Proportional to storage writes
**Emits:** `VoteCast` event with voter, puzzle_id, scores, and weight

**Constraints:**
- All scores must be 1-5 (inclusive)
- Voter must have >= min_stake_threshold staked tokens
- One vote per (voter, puzzle_id) pair - duplicates rejected

**Example:**
```rust
// Vote that puzzle #42 has difficulty 3, is fun (4), and fairly balanced (4)
puzzle_voting::cast_vote(
    env,
    voter_address,
    42,  // puzzle_id
    3,   // difficulty (1-5)
    4,   // fun (1-5)
    4    // fairness (1-5)
)
```

#### get_vote
```rust
fn get_vote(env: Env, voter: Address, puzzle_id: u32) -> Option<PuzzleVote>
```

Retrieve a specific vote cast by a voter on a puzzle.

**Returns:** The PuzzleVote struct if it exists, None otherwise

**Example:**
```rust
let vote = puzzle_voting::get_vote(env, voter_addr, puzzle_id);
if let Some(v) = vote {
    println!("Voted difficulty: {}", v.difficulty_score);
    println!("Voting weight: {}", v.weight);
}
```

#### get_aggregate
```rust
fn get_aggregate(env: Env, puzzle_id: u32) -> Option<PuzzleVotingAggregate>
```

Get aggregated voting results for a puzzle.

**Returns:** Weighted averages and vote count if votes exist

**Example:**
```rust
if let Some(agg) = puzzle_voting::get_aggregate(env, puzzle_id) {
    let difficulty = agg.weighted_difficulty_avg / 1000;  // Scale down
    println!("Average difficulty: {}", difficulty);
    println!("Total votes: {}", agg.vote_count);
    println!("Total weight: {}", agg.total_weight);
}
```

### Admin Functions

#### reset_puzzle_votes
```rust
fn reset_puzzle_votes(env: Env, puzzle_id: u32)
```

Reset all votes for a puzzle (requires admin authorization).

**Use cases:**
- Puzzle was edited/corrected
- Votes contain invalid data
- Need to recollect fresh feedback

**Emits:** `VotesReset` event

#### update_min_stake_threshold
```rust
fn update_min_stake_threshold(env: Env, new_threshold: i128)
```

Update the minimum stake required to vote (requires admin authorization).

**Example:**
```rust
// Increase minimum stake to 500 tokens
puzzle_voting::update_min_stake_threshold(env, 500);
```

## Weighted Averaging Algorithm

The contract uses a weighted average formula where each vote is weighted by the voter's staked balance:

```
Weighted Average = (sum of (score × voter_weight)) / (sum of all voter_weights)
```

**Example:**
- Voter A (1000 tokens staked): votes difficulty = 3
- Voter B (2000 tokens staked): votes difficulty = 5

Weighted Average = (3×1000 + 5×2000) / (1000 + 2000) = 13000 / 3000 = 4.33

### Precision Handling

Scores are scaled by 1000 before weighted averaging to maintain precision in fixed-point calculations:

```
stored_value = (score * 1000 * weight) / total_weight
actual_score = stored_value / 1000
```

## Events

### VoteCast
```rust
VoteCast {
    voter: Address,
    puzzle_id: u32,
    difficulty_score: u32,
    fun_score: u32,
    fairness_score: u32,
    weight: i128,
}
```

Emitted when a vote is successfully cast.

### VotesReset
```rust
VotesReset {
    puzzle_id: u32,
    reset_at: u64,
}
```

Emitted when votes for a puzzle are reset (admin-only).

### MinStakeThresholdUpdated
```rust
MinStakeThresholdUpdated {
    new_threshold: i128,
    updated_at: u64,
}
```

Emitted when the minimum stake threshold is updated (admin-only).

## Error Handling

All errors are thrown as panics with clear messages:

| Error | Cause |
|-------|-------|
| "Already initialized" | initialize() called more than once |
| "Difficulty score must be between 1 and 5" | Invalid difficulty score |
| "Fun score must be between 1 and 5" | Invalid fun score |
| "Fairness score must be between 1 and 5" | Invalid fairness score |
| "Voter has already voted on this puzzle" | Duplicate vote detected |
| "Voter does not meet minimum stake threshold" | Insufficient staked tokens |
| "Contract not initialized" | Operations before initialize() |

## Integration with Staking Contract

The voting contract queries the staking contract to determine each voter's weight:

```
voter_weight = staking_contract.get_staker_balance(voter)
```

The staking contract must provide a public function to retrieve a voter's staked balance. The integration is critical for the weighting mechanism.

### Implementation Note

The current implementation requires updating the `get_voter_weight()` function to properly call the staking contract's balance retrieval function once the staking contract interface is fully defined.

## Security Considerations

1. **Authentication**: All mutative operations require proper authorization
2. **Validation**: Score ranges strictly enforced (1-5)
3. **Stake Checking**: Minimum stake threshold prevents low-token holders from voting
4. **Immutable Records**: Individual votes cannot be modified after casting
5. **Admin Reset**: Only authorized admin can alter aggregate data

## Testing

The contract includes comprehensive tests covering:

- ✅ Initialization
- ✅ Valid vote casting with score boundaries
- ✅ Invalid score rejection (0, 6, etc.)
- ✅ Storage operations for votes and aggregates
- ✅ Duplicate vote detection
- ✅ Single-vote weighted averages
- ✅ Multi-vote weighted averaging
- ✅ Vote count tracking
- ✅ Configuration management
- ✅ Vote reset functionality
- ✅ Minimum stake enforcement
- ✅ Score boundary validation

### Running Tests

```bash
cargo test puzzle_voting
```

## Deployment Checklist

- [ ] Staking contract address obtained
- [ ] Admin address determined
- [ ] Minimum stake threshold configured
- [ ] All tests passing
- [ ] Contract built with testnet configuration
- [ ] Deployed to testnet
- [ ] initialization() called with correct parameters
- [ ] Sample votes cast and verified
- [ ] Aggregate calculations validated
- [ ] Event emission confirmed via testnet logs

## Future Enhancements

1. **Vote Weighting Options**: Allow puzzle creators to weight by puzzle difficulty
2. **Time-Locked Voting**: Prevent new votes after puzzle deadline
3. **Vote Modification**: Allow voters to update votes within time window
4. **Sentiment Analysis**: Add confidence/certainty weight to votes
5. **Batched Aggregation**: Process multiple puzzles in single transaction
6. **Historical Voting**: Track voting trends over time
7. **Delegation**: Allow voting power delegation to other addresses

## Gas Optimization Tips

1. Batch vote resets using external aggregation script
2. Cache aggregate values in off-chain database
3. Use lazy evaluation for historical scores
4. Index votes by puzzle for efficient querying

## References

- [Soroban SDK Documentation](https://github.com/stellar/rs-soroban-sdk)
- [Stellar Smart Contracts](https://developers.stellar.org/docs/learn/smart-contracts)
