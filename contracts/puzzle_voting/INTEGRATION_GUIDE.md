# Puzzle Voting Contract - Integration Guide

## Overview

This guide explains how to integrate the Puzzle Voting contract with the Staking contract to enable weighted voting on puzzles.

## Architecture Overview

```
┌─────────────────────────┐
│  Game Frontend/Backend  │
└────────────┬────────────┘
             │ calls
             ▼
┌─────────────────────────────────────────┐
│  Puzzle Voting Contract                 │
│  - cast_vote()                          │
│  - get_aggregate()                      │
│  - get_vote()                           │
│  - reset_puzzle_votes()                 │
└────────────┬────────────────────────────┘
             │ cross-contract call
             │ get_voter_weight()
             ▼
┌─────────────────────────┐
│  Staking Contract       │
│  - get_staker_info()    │
└─────────────────────────┘
```

## Implementation Steps

### 1. Deploy Staking Contract First

Ensure the staking contract is deployed to testnet and you have its contract address:

```bash
soroban contract deploy \
  --wasm-path target/wasm32-unknown-unknown/release/staking.wasm \
  --network testnet
# Note the contract ID (e.g., CAB...XYZ)
```

### 2. Update Puzzle Voting Contract's get_voter_weight()

The staking contract provides a public `get_staker_info()` function that returns:

```rust
pub struct StakerInfo {
    pub staked_amount: i128,        // This is the voting weight we need
    pub stake_timestamp: u64,
    pub last_reward_claim: u64,
    pub accumulated_rewards: i128,
    pub tier: StakingTier,
}
```

Update the `get_voter_weight()` function in `src/lib.rs`:

```rust
/// Get a voter's staked balance as voting weight
fn get_voter_weight(env: &Env, voter: &Address, config: &VotingConfig) -> i128 {
    // Query staking contract for voter's staked balance
    // Returns the staked amount which is used as voting weight
    
    let args: Vec<Val> = vec![env, voter.clone().into_val(env)];
    
    match env.invoke_contract::<Option<StakerInfo>>(
        &config.staking_contract,
        &Symbol::new(env, "get_staker_info"),
        args,
    ) {
        Some(staker_info) => staker_info.staked_amount,
        None => 0i128,
    }
}
```

**Important:** You'll need to define `StakerInfo` in `src/types.rs` or import it from the staking contract if that's possible in your Soroban setup.

### 3. Deploy Puzzle Voting Contract

After updating the `get_voter_weight()` function, rebuild and deploy:

```bash
cargo build --target wasm32-unknown-unknown --release

soroban contract deploy \
  --wasm-path target/wasm32-unknown-unknown/release/puzzle_voting.wasm \
  --network testnet
# Note the contract ID (e.g., CDY...ABC)
```

### 4. Initialize the Puzzle Voting Contract

Call the `initialize()` function with the correct parameters:

```bash
soroban contract invoke \
  --network testnet \
  --id CDY...ABC \
  -- initialize \
  --admin GXXXXXXX... \
  --staking_contract CAB...XYZ \
  --min_stake_threshold 100000000  # Minimum 1 token (with 8 decimals)
```

**Parameters:**
- `admin`: Address that can reset votes and update thresholds
- `staking_contract`: Contract ID of the deployed staking contract (e.g., CAB...XYZ)
- `min_stake_threshold`: Minimum staked tokens to vote (e.g., 100000000 = 1 token with 8 decimals)

### 5. Testing the Integration

#### Test 1: Cast a Vote

First, ensure a player has staked tokens:

```bash
# Check staker info from staking contract
soroban contract invoke \
  --network testnet \
  --id CAB...XYZ \
  -- get_staker_info \
  --staker GXXXXXXXXX...
```

Then cast a vote:

```bash
soroban contract invoke \
  --network testnet \
  --id CDY...ABC \
  -- cast_vote \
  --voter GXXXXXXXXX... \
  --puzzle_id 1 \
  --difficulty 3 \
  --fun 4 \
  --fairness 3
```

#### Test 2: Get Aggregated Results

```bash
soroban contract invoke \
  --network testnet \
  --id CDY...ABC \
  -- get_aggregate \
  --puzzle_id 1
```

Expected output:
```
{
  puzzle_id: 1,
  weighted_difficulty_avg: 3000,  # 3.0 (scaled by 1000)
  weighted_fun_avg: 4000,          # 4.0 (scaled by 1000)
  weighted_fairness_avg: 3000,     # 3.0 (scaled by 1000)
  vote_count: 1,
  total_weight: 100000000,         # 1 token
  is_reset: false,
  last_reset_at: 0
}
```

#### Test 3: Weight Validation

Test that a voter with insufficient stake is rejected:

1. Create a new account with less than min_stake_threshold
2. Try to vote
3. Should get error: "Voter does not meet minimum stake threshold"

### 6. Integration with Game Backend

Your game backend should follow this flow:

```typescript
import * as SorobanClient from "soroban-js-sdk"; // or similar

async function submitPuzzleVote(
  puzzleId: number,
  voterId: string,
  difficulty: number,
  fun: number,
  fairness: number
) {
  const contract = new SorobanClient.Contract(
    PUZZLE_VOTING_CONTRACT_ID, // CDY...ABC
    server
  );

  // Call cast_vote
  const vote = await contract.methods
    .cast_vote({
      voter: voterId,
      puzzle_id: puzzleId,
      difficulty,
      fun,
      fairness,
    })
    .call();

  return vote;
}

async function getPuzzleScores(puzzleId: number) {
  const contract = new SorobanClient.Contract(
    PUZZLE_VOTING_CONTRACT_ID, // CDY...ABC
    server
  );

  const aggregate = await contract.methods
    .get_aggregate({ puzzle_id: puzzleId })
    .call();

  if (!aggregate) {
    return null;
  }

  // Scale down from precision (divided by 1000)
  return {
    difficulty: aggregate.weighted_difficulty_avg / 1000,
    fun: aggregate.weighted_fun_avg / 1000,
    fairness: aggregate.weighted_fairness_avg / 1000,
    voteCount: aggregate.vote_count,
    totalWeight: aggregate.total_weight,
  };
}
```

## Data Synchronization

### Off-Chain Indexing

Due to Soroban's storage model, consider maintaining an off-chain database of votes for quick queries:

```sql
CREATE TABLE puzzle_votes (
  id INT PRIMARY KEY,
  puzzle_id INT,
  voter VARCHAR(56),
  difficulty_score INT,
  fun_score INT,
  fairness_score INT,
  weight BIGINT,
  voted_at BIGINT,
  tx_hash VARCHAR(64),
  block_height INT,
  indexed_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE puzzle_aggregates (
  puzzle_id INT PRIMARY KEY,
  weighted_difficulty_avg NUMERIC(20,0),
  weighted_fun_avg NUMERIC(20,0),
  weighted_fairness_avg NUMERIC(20,0),
  vote_count INT,
  total_weight BIGINT,
  is_reset BOOLEAN,
  last_reset_at BIGINT,
  updated_at TIMESTAMP DEFAULT NOW()
);
```

Subscribe to `VoteCast` and `VotesReset` events to update your database in real-time.

## Performance Considerations

### Transaction Costs

Each vote cast incurs:
- Data storage costs for Vote record
- Computation for weighted average calculation
- Cross-contract call to staking contract

Typical costs per vote: ~0.01-0.05 XLM (varies with network)

### Query Performance

#### On-Chain Queries
- `get_vote()` - O(1) lookup
- `get_aggregate()` - O(1) lookup
- `get_vote_count()` - O(1) lookup

#### Off-Chain Queries
Use indexed database for:
- Votes by puzzle
- Votes by voter
- Votes by timestamp range
- Voting trends

### Batch Operations

For admin operations like vote resets:
- Only reset when necessary (puzzle edits)
- Consider batching multiple puzzle resets if possible
- Plan resets during low-traffic periods

## Common Issues and Troubleshooting

### Issue: "Voter does not meet minimum stake threshold"

**Cause:** Voter's staked amount is less than `min_stake_threshold`

**Solution:**
1. Verify staker's balance: `soroban contract invoke --id CAB...XYZ -- get_staker_info --staker GXXXXX...`
2. Check min_stake_threshold: Store record during initialization
3. Have voter stake more tokens
4. Or reduce min_stake_threshold if appropriate

### Issue: "Voter has already voted on this puzzle"

**Cause:** The voter already cast a vote on this puzzle

**Solution:**
- Current implementation: Cannot modify/delete votes
- Future: Allow vote updates within time window
- For now: Ask player to wait for admin reset if they voted incorrectly

### Issue: Cross-contract call returns error

**Cause:** Staking contract not deployed or address incorrect

**Solution:**
1. Verify staking contract is deployed
2. Double-check staking contract ID in initialization
3. Ensure staking contract has public `get_staker_info()` function
4. Check contract addresses in logs

### Issue: Aggregations don't match expected values

**Cause:** Floating-point precision or incorrect weight calculation

**Solution:**
- Aggregates use 1000x scaling for precision
- Divide by 1000 when displaying to users
- Verify weights via `get_vote()` call
- Check vote counts and total weights

## Security Checklist

- [ ] Admin address is secure (not a test key)
- [ ] Staking contract is audited
- [ ] Minimum stake threshold is appropriate (not too low)
- [ ] Contract addresses are verified before initialization
- [ ] Cross-contract calls properly handle errors
- [ ] Events are logged and monitored
- [ ] Voting period is appropriate for game balance
- [ ] Admin reset capability is monitored

## Updating the Contract

When updating the contract:

1. Do NOT change storage structure (breaks existing votes)
2. Test all aggregate calculations with existing votes
3. Test migration of old votes to new format if needed
4. Consider versioning the contract address

## References

- [Puzzle Voting Contract README](./README.md)
- [Staking Contract Documentation](../staking/README.md)
- [Soroban Cross-Contract Calls](https://developers.stellar.org/docs/learn/smart-contracts/contract-interactions)
- [Soroban Storage Model](https://developers.stellar.org/docs/learn/smart-contracts/storing-data)
