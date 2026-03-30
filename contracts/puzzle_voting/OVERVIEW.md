# Puzzle Voting Contract - Quick Overview

## What Is This?

A smart contract that lets players vote on puzzles using their staked tokens as voting power. Each vote rates a puzzle on three dimensions: **difficulty accuracy**, **fun factor**, and **fairness**. Puzzle creators and the game backend get weighted feedback to improve game balance.

## How It Works (3 Minutes)

### 1. Player Votes
```
Player (with 1000 staked tokens) votes on Puzzle #5:
- Difficulty: 3 out of 5 ⭐⭐⭐
- Fun:        4 out of 5 ⭐⭐⭐⭐
- Fairness:   3 out of 5 ⭐⭐⭐

Voting Power: 1000 tokens (from staking contract)
```

### 2. Vote Is Recorded
Contract stores the vote on-chain with:
- Who voted (voter address)
- Which puzzle (puzzle_id)
- Their three scores
- Their voting weight (staked balance)
- Timestamp

### 3. Game Gets Results
```
Aggregate for Puzzle #5:
- Average Difficulty: 3.2/5 (from all votes)
- Average Fun:        3.8/5
- Average Fairness:   3.1/5
- Total Votes:        127
- Total Weight:       850,000 tokens
```

Result is queryable on-chain for game backend to use.

## Key Features

| Feature | What It Does |
|---------|-------------|
| **Token-Weighted Voting** | Your vote counts more if you have more staked tokens |
| **Multi-Dimensional** | Vote on 3 different aspects (difficulty, fun, fairness) |
| **No Duplicates** | Each player votes once per puzzle (can't vote twice) |
| **Configurable Threshold** | Admin sets minimum stake required to vote |
| **Vote Reset** | Admin can clear votes if puzzle is edited |
| **On-Chain Events** | Voting actions are broadcast as events |
| **Queryable Results** | Game backend can instantly get aggregate scores |

## Technical Specs

```
Contract Type:     Soroban Smart Contract (Stellar)
Language:          Rust
Storage:           On-chain persistent storage
Weighted Avg:      Calculated using fixed-point arithmetic (×1000 scaling)
Cross-Contract:    Integrates with Staking contract
Events:            VoteCast, VotesReset, MinStakeThresholdUpdated
```

## Getting Started

### For Deployers
1. Read [DEPLOYMENT_CHECKLIST.md](./DEPLOYMENT_CHECKLIST.md) for step-by-step deployment
2. Have testnet LUMENS ready
3. Know your staking contract address
4. Deploy and initialize

### For Developers
1. Read [INTEGRATION_GUIDE.md](./INTEGRATION_GUIDE.md) to understand integration
2. See code examples in [README.md](./README.md)
3. Review [src/lib.rs](./src/lib.rs) for implementation details

### For Users
1. Stake tokens in staking contract
2. Vote on puzzles you've played
3. See aggregate scores help improve game balance

## File Structure

```
puzzle_voting/
├── src/
│   ├── lib.rs       - Main contract (290+ lines)
│   ├── types.rs     - Data structures (100+ lines)
│   └── storage.rs   - Storage layer (80+ lines)
├── Cargo.toml       - Package config
├── README.md        - Full user documentation
├── INTEGRATION_GUIDE.md - Integration instructions
├── DEPLOYMENT_CHECKLIST.md - Deployment steps
├── IMPLEMENTATION_SUMMARY.md - What was built
└── OVERVIEW.md      - This file
```

## Public Functions

### For Players
```rust
cast_vote(voter, puzzle_id, difficulty, fun, fairness)
  → Casts a vote on a puzzle
  
get_vote(voter, puzzle_id) 
  → Returns your vote (if you've voted)
  
get_aggregate(puzzle_id)
  → Returns all votes combined (average scores)
```

### For Admins
```rust
update_min_stake_threshold(new_threshold)
  → Changes minimum tokens needed to vote
  
reset_puzzle_votes(puzzle_id)
  → Clears all votes for a puzzle
```

## Data Flow

```
┌─────────────┐
│   Player    │ Votes on puzzle
└────┬────────┘
     │
     ↓
┌─────────────────────────────────────────┐
│  Puzzle Voting Contract                 │
│  - Checks player has staked enough      │
│  - Checks player hasn't voted yet       │
│  - Validates scores are 1-5             │
│  - Gets voting weight from staking      │
│  - Stores vote on-chain                 │
│  - Updates weighted averages            │
└────┬────────────────────────────────────┘
     │
     ↓
┌──────────────────────────┐
│  Game Backend/Frontend   │
│  - Queries get_aggregate │
│  - Displays scores       │
│  - Uses for rebalancing  │
└──────────────────────────┘
```

## Example Scenario

**Setup:**
- Puzzle #10: "Tricky Math Challenge"
- 3 players have staked tokens

**Votes Cast:**
```
Player A (1000 tokens): difficulty=4, fun=5, fairness=4
Player B (2000 tokens): difficulty=3, fun=4, fairness=3
Player C (1000 tokens): difficulty=5, fun=4, fairness=5
```

**Weighted Aggregates:**
```
Total tokens: 4000
Difficulty: (4×1000 + 3×2000 + 5×1000) / 4000 = 3.75
Fun:        (5×1000 + 4×2000 + 4×1000) / 4000 = 4.25
Fairness:   (4×1000 + 3×2000 + 5×1000) / 4000 = 3.75

Result: Puzzle #10 is moderately difficult (3.75), very fun (4.25), and fairly balanced (3.75)
```

The game designer sees Player B (who has 2× voting weight) voted difficulty=3, so the puzzle is slightly easier than average but the community agrees it's fun. ✅

## Score Scale

All ratings use a 1-5 scale:

```
1 ⭐      - Poor / Very Easy / Unfair
2 ⭐⭐    - Below Average / Easy / Slightly Unfair
3 ⭐⭐⭐  - Average / Medium / Fair
4 ⭐⭐⭐⭐ - Good / Hard / Well-Balanced
5 ⭐⭐⭐⭐⭐ - Excellent / Very Hard / Very Fair
```

## Storage Costs

Approximate costs per operation (varies with Soroban network):

| Operation | Cost |
|-----------|------|
| cast_vote | 0.01-0.05 XLM |
| get_aggregate | Free (read) |
| get_vote | Free (read) |
| reset_votes | 0.001 XLM |
| update_threshold | 0.001 XLM |

## Security Notes

✅ **Secure by default:**
- Only authenticated voters can vote
- Only admin can reset votes
- Votes are immutable (can't change after casting)
- Minimum stake prevents spam
- Clear error messages for validation failures

⚠️ **Things to know:**
- Players cannot update their votes (vote once per puzzle)
- Reset clears ALL votes for a puzzle
- Admin should be a secure address (not test key)

## Common Questions

**Q: Can I change my vote?**
A: No, once you vote it's locked in. If you voted wrong, ask the admin to reset all votes so you can vote again.

**Q: What if I have 0 staked tokens?**
A: You can't vote. You need to stake first in the staking contract.

**Q: Why are my scores scaled by 1000?**
A: To maintain precision in weighted averages. Divide by 1000 to get the real score (3000 = 3.0).

**Q: Can I see who voted what?**
A: Yes, call `get_vote(voter_address, puzzle_id)` to see a specific player's vote.

**Q: What if a puzzle is edited?**
A: Admin calls `reset_puzzle_votes(puzzle_id)` to clear votes, then players can vote again.

## Next Steps

1. **Deploy**: Follow [DEPLOYMENT_CHECKLIST.md](./DEPLOYMENT_CHECKLIST.md)
2. **Integrate**: Follow [INTEGRATION_GUIDE.md](./INTEGRATION_GUIDE.md)
3. **Test**: Cast sample votes and verify aggregates
4. **Launch**: Announce to players!

## Support

For implementation questions, see:
- **API Details**: [README.md](./README.md)
- **Integration Help**: [INTEGRATION_GUIDE.md](./INTEGRATION_GUIDE.md)
- **Deployment Guide**: [DEPLOYMENT_CHECKLIST.md](./DEPLOYMENT_CHECKLIST.md)
- **Code**: [src/lib.rs](./src/lib.rs)

---

**Status**: ✅ Ready for Testnet Deployment
**Difficulty**: 3/5 - Medium (integration required)
**Estimated Integration Time**: 4-8 hours
