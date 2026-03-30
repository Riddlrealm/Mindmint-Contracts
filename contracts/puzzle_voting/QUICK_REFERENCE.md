# Puzzle Voting Contract - Quick Reference

## Function Signatures

### Initialization

```rust
pub fn initialize(
    env: Env,
    admin: Address,
    staking_contract: Address,
    min_stake_threshold: i128,
)
```
- **Purpose**: Initialize contract (call once only)
- **Admin**: Public
- **Parameters**: admin address, staking contract address, minimum stake in tokens
- **Example**: `initialize(env, admin_addr, stake_contract_addr, 100000000)`

---

### Voting Functions

#### cast_vote()
```rust
pub fn cast_vote(
    env: Env,
    voter: Address,
    puzzle_id: u32,
    difficulty: u32,   // 1-5
    fun: u32,          // 1-5
    fairness: u32,     // 1-5
)
```
- **Purpose**: Cast vote on a puzzle
- **Requires**: Voter authentication, staked balance ≥ threshold
- **Constraints**: One vote per (voter, puzzle_id), scores must be 1-5
- **Returns**: Void (emits VoteCast event)
- **Example**: `cast_vote(env, voter, 42, 3, 4, 3)`
- **Error Cases**:
  - "Already initialized" (if called before init)
  - "Difficulty/Fun/Fairness score must be between 1 and 5"
  - "Voter has already voted on this puzzle"
  - "Voter does not meet minimum stake threshold"

#### get_vote()
```rust
pub fn get_vote(
    env: Env,
    voter: Address,
    puzzle_id: u32,
) -> Option<PuzzleVote>
```
- **Purpose**: Retrieve a specific vote
- **Requires**: Nothing (public read)
- **Returns**: PuzzleVote if exists, None otherwise
- **Example**: `if let Some(vote) = get_vote(env, voter_addr, 42) { ... }`

**PuzzleVote Structure**:
```rust
struct PuzzleVote {
    voter: Address,              // Who voted
    puzzle_id: u32,              // Which puzzle
    difficulty_score: u32,       // 1-5
    fun_score: u32,              // 1-5
    fairness_score: u32,         // 1-5
    weight: i128,                // Their staked balance
    voted_at: u64,               // Timestamp
}
```

#### get_aggregate()
```rust
pub fn get_aggregate(
    env: Env,
    puzzle_id: u32,
) -> Option<PuzzleVotingAggregate>
```
- **Purpose**: Get all votes combined for a puzzle
- **Requires**: Nothing (public read)
- **Returns**: Aggregates if votes exist, None otherwise
- **Example**: `if let Some(agg) = get_aggregate(env, 42) { ... }`

**PuzzleVotingAggregate Structure**:
```rust
struct PuzzleVotingAggregate {
    puzzle_id: u32,                    // Which puzzle
    weighted_difficulty_avg: u128,     // Scaled by 1000
    weighted_fun_avg: u128,            // Scaled by 1000
    weighted_fairness_avg: u128,       // Scaled by 1000
    vote_count: u32,                   // Total votes
    total_weight: i128,                // Sum of voter weights
    is_reset: bool,                    // Was reset?
    last_reset_at: u64,                // Reset timestamp
}
```

**Note**: Divide averages by 1000 to get actual scores:
```
actual_difficulty = weighted_difficulty_avg / 1000
// e.g., 3500 / 1000 = 3.5
```

---

### Admin Functions

#### update_min_stake_threshold()
```rust
pub fn update_min_stake_threshold(
    env: Env,
    new_threshold: i128,
)
```
- **Purpose**: Change minimum stake required to vote
- **Requires**: Admin authentication only
- **Returns**: Void (emits MinStakeThresholdUpdated event)
- **Example**: `update_min_stake_threshold(env, 50000000)` // 0.5 tokens
- **Error Cases**:
  - "Minimum stake threshold must be non-negative"
  - Authentication error if not admin

#### reset_puzzle_votes()
```rust
pub fn reset_puzzle_votes(
    env: Env,
    puzzle_id: u32,
)
```
- **Purpose**: Clear all votes for a puzzle
- **Requires**: Admin authentication only
- **Returns**: Void (emits VotesReset event)
- **Example**: `reset_puzzle_votes(env, 42)`
- **Use Cases**: After puzzle edit, to fix incorrect votes
- **Note**: Players can vote again after reset

---

## Event Types

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
Emitted when: Player casts a vote

### VotesReset
```rust
VotesReset {
    puzzle_id: u32,
    reset_at: u64,
}
```
Emitted when: Admin resets votes for a puzzle

### MinStakeThresholdUpdated
```rust
MinStakeThresholdUpdated {
    new_threshold: i128,
    updated_at: u64,
}
```
Emitted when: Admin updates minimum stake

---

## Error Messages

| Message | Cause | Solution |
|---------|-------|----------|
| "Already initialized" | Called init twice | Only call once at deployment |
| "Contract not initialized" | Called function before init | Initialize first |
| "Difficulty/Fun/Fairness score must be between 1 and 5" | Score out of range | Use 1-5 only |
| "Voter has already voted on this puzzle" | Duplicate vote | (Can't fix - ask admin for reset) |
| "Voter does not meet minimum stake threshold" | Not enough tokens staked | Stake more tokens |
| "Minimum stake threshold must be non-negative" | Invalid threshold | Use value ≥ 0 |

---

## Usage Examples

### Example 1: Cast a Vote
```bash
soroban contract invoke \
  --id CDY...ABC \
  -- cast_vote \
  --voter GXXXXXXXXX... \
  --puzzle_id 5 \
  --difficulty 4 \
  --fun 4 \
  --fairness 3
```

### Example 2: Get Your Vote
```bash
soroban contract invoke \
  --id CDY...ABC \
  -- get_vote \
  --voter GXXXXXXXXX... \
  --puzzle_id 5
```

### Example 3: Get Aggregate Scores
```bash
soroban contract invoke \
  --id CDY...ABC \
  -- get_aggregate \
  --puzzle_id 5
```

### Example 4: Admin Reset Votes
```bash
soroban contract invoke \
  --id CDY...ABC \
  --source-account ADMIN_KEY \
  -- reset_puzzle_votes \
  --puzzle_id 5
```

### Example 5: Admin Update Threshold
```bash
soroban contract invoke \
  --id CDY...ABC \
  --source-account ADMIN_KEY \
  -- update_min_stake_threshold \
  --new_threshold 200000000
```

---

## Data Types

### Score (1-5 scale)
- Type: `u32`
- Valid: 1, 2, 3, 4, 5
- Invalid: 0, 6, -1, etc.

### Puzzle ID
- Type: `u32`
- Range: 0 to 4,294,967,295
- Example: `42`

### Address
- Type: `Address` (Stellar address)
- Format: `GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX`
- Example: `GBUQWP3BOUZX34ULNQG23RQ6F4OFSAI5DVKEDCNQSXNgavkztkinlpep`

### Token Amount
- Type: `i128`
- Unit: Smallest unit (with decimals)
- Unit: For 8-decimal token: 100000000 = 1 token
- Example: `min_stake = 100000000` means 1 token minimum

### Timestamp
- Type: `u64`
- Unit: Seconds since Unix epoch
- Example: `1711814400` = March 30, 2024

### Weight
- Type: `i128`
- Meaning: Voter's staked token balance at vote time
- Example: `1000000000` = 10 tokens (8 decimals)

---

## Constants

| Name | Value | Meaning |
|------|-------|---------|
| MIN_SCORE | 1 | Lowest valid score |
| MAX_SCORE | 5 | Highest valid score |
| PRECISION_SCALE | 1000 | Scale factor for weighted averages |

---

## Storage Keys

Internal storage organization (for reference):

```
Instance Storage (contract config):
└── Config

Persistent Storage (votes/aggregates):
├── Vote(voter: Address, puzzle_id: u32)
├── Aggregate(puzzle_id: u32)
└── VoteCount(puzzle_id: u32)
```

---

## Integration Checklist

- [ ] Deploy puzzle_voting contract
- [ ] Deploy staking contract (must be first)
- [ ] Call initialize() with:
  - [ ] Admin address (secure)
  - [ ] Staking contract address
  - [ ] Min stake threshold
- [ ] Test cast_vote() works
- [ ] Test get_vote() returns correct data
- [ ] Test get_aggregate() calculations
- [ ] Test reset_puzzle_votes() (admin)
- [ ] Test update_min_stake_threshold() (admin)
- [ ] Verify events emit correctly
- [ ] Test error cases
- [ ] Ready for production!

---

## Performance Notes

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| cast_vote | O(1) | O(1) |
| get_vote | O(1) | O(1) |
| get_aggregate | O(1) | O(1) |
| reset_votes | O(1) | O(1) |
| update_threshold | O(1) | O(1) |

---

## Version Info

- **Contract Version**: 0.1.0
- **Soroban SDK**: 21.0.0
- **Edition**: 2021
- **Created**: March 30, 2026

---

## More Help

- **Full Docs**: See [README.md](./README.md)
- **Integration**: See [INTEGRATION_GUIDE.md](./INTEGRATION_GUIDE.md)
- **Deployment**: See [DEPLOYMENT_CHECKLIST.md](./DEPLOYMENT_CHECKLIST.md)
- **Overview**: See [OVERVIEW.md](./OVERVIEW.md)
- **Code**: See [src/lib.rs](./src/lib.rs)
