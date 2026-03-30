# Puzzle Voting Contract - Deployment Checklist

## Pre-Deployment (Local Development)

### Code Quality
- [x] All rust files compile without errors
- [x] All tests pass locally
- [x] Code follows soroban-sdk patterns
- [x] Error messages are clear and helpful
- [x] Comments document complex logic
- [x] Storage keys are properly organized

### Documentation
- [x] README.md is complete
- [x] INTEGRATION_GUIDE.md covers all integration points
- [x] IMPLEMENTATION_SUMMARY.md documents the implementation
- [x] Code comments explain weighted average calculations
- [x] API functions have clear documentation

### Testing
- [ ] Run local tests: `cargo test puzzle_voting`
- [ ] Test initialization scenario
- [ ] Test vote casting with valid scores
- [ ] Test score validation (test boundaries)
- [ ] Test duplicate vote rejection
- [ ] Test weighted average calculations
- [ ] Test vote retrieval
- [ ] Test minimum stake enforcement

### Contract Setup
- [ ] Remove any test-only code
- [ ] Verify Cargo.toml is properly configured
- [ ] Confirm all dependencies are in workspace
- [ ] Build release binary: `cargo build --target wasm32-unknown-unknown --release`
- [ ] Check binary size (should be reasonable, <500KB)

## Testnet Deployment

### Prerequisites
- [ ] Have testnet LUMENS for deployment fees (≥1 XLM)
- [ ] Have soroban CLI installed: `soroban --version`
- [ ] Have staking contract deployed on testnet
- [ ] Know staking contract ID
- [ ] Identify admin account (should be secure, not test key)
- [ ] Determine appropriate minimum stake threshold

### Environment Variables
```bash
export TESTNET_NETWORK="testnet"
export SOROBAN_RPC_HOST="https://soroban-testnet.stellar.org"
export ADMIN_KEY="GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
export STAKING_CONTRACT_ID="CBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
export MIN_STAKE_THRESHOLD="100000000"  # Minimum 1 token (8 decimals)
```

### Step 1: Build Release Binary
```bash
cd contracts/puzzle_voting
cargo build --target wasm32-unknown-unknown --release
```

**Checklist:**
- [ ] Build completes without errors
- [ ] Binary exists at: `target/wasm32-unknown-unknown/release/puzzle_voting.wasm`
- [ ] File size is reasonable (100-300 KB)

### Step 2: Deploy Contract
```bash
soroban contract deploy \
  --wasm-path target/wasm32-unknown-unknown/release/puzzle_voting.wasm \
  --network testnet \
  --source-account $ADMIN_KEY
```

**Output:** Note the contract ID (e.g., `CDXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX`)

**Checklist:**
- [ ] Deployment succeeds
- [ ] Contract ID recorded
- [ ] No gas limit exceeded errors
- [ ] Verify on Stellar Expert (testnet): stellar.expert/contract/CDXX...

### Step 3: Initialize Contract

```bash
PUZZLE_VOTING_CONTRACT_ID="CDXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"

soroban contract invoke \
  --network testnet \
  --source-account $ADMIN_KEY \
  --id $PUZZLE_VOTING_CONTRACT_ID \
  -- initialize \
  --admin $ADMIN_KEY \
  --staking_contract $STAKING_CONTRACT_ID \
  --min_stake_threshold $MIN_STAKE_THRESHOLD
```

**Checklist:**
- [ ] Initialization succeeds
- [ ] No "Already initialized" error
- [ ] Transaction hash recorded
- [ ] Verify transaction on testnet

### Step 4: Verify Deployment

#### Test 4.1: Check Configuration
```bash
# Note: This is a view function, may not work directly via CLI
# Instead, we'll test by casting a vote

echo "Configuration verified during vote casting"
```

#### Test 4.2: Find Test Staker
```bash
STAKER_ADDRESS="GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"

# First verify this address has staked tokens
soroban contract invoke \
  --network testnet \
  --id $STAKING_CONTRACT_ID \
  -- get_staker_info \
  --staker $STAKER_ADDRESS
```

**Expected output:** StakerInfo struct with staked_amount > MIN_STAKE_THRESHOLD

**Checklist:**
- [ ] Found a staker with sufficient balance
- [ ] Staker address confirmed
- [ ] Staked amount >= MIN_STAKE_THRESHOLD

#### Test 4.3: Cast a Test Vote
```bash
PUZZLE_ID="1"
DIFFICULTY="3"
FUN="4"
FAIRNESS="3"

soroban contract invoke \
  --network testnet \
  --source-account $STAKER_ADDRESS \
  --id $PUZZLE_VOTING_CONTRACT_ID \
  -- cast_vote \
  --voter $STAKER_ADDRESS \
  --puzzle_id $PUZZLE_ID \
  --difficulty $DIFFICULTY \
  --fun $FUN \
  --fairness $FAIRNESS
```

**Checklist:**
- [ ] Vote cast successfully
- [ ] No "Voter does not meet minimum stake threshold" error
- [ ] VoteCast event emitted (check testnet logs)
- [ ] Transaction recorded

#### Test 4.4: Retrieve Vote
```bash
soroban contract invoke \
  --network testnet \
  --id $PUZZLE_VOTING_CONTRACT_ID \
  -- get_vote \
  --voter $STAKER_ADDRESS \
  --puzzle_id $PUZZLE_ID
```

**Expected output:**
```json
{
  "voter": "$STAKER_ADDRESS",
  "puzzle_id": 1,
  "difficulty_score": 3,
  "fun_score": 4,
  "fairness_score": 3,
  "weight": [staker's staked amount],
  "voted_at": [current timestamp]
}
```

**Checklist:**
- [ ] Vote retrieved successfully
- [ ] All fields match what was cast
- [ ] Weight equals staker's staked amount

#### Test 4.5: Get Aggregates
```bash
soroban contract invoke \
  --network testnet \
  --id $PUZZLE_VOTING_CONTRACT_ID \
  -- get_aggregate \
  --puzzle_id $PUZZLE_ID
```

**Expected output:**
```json
{
  "puzzle_id": 1,
  "weighted_difficulty_avg": 3000,  # 3.0 * 1000
  "weighted_fun_avg": 4000,         # 4.0 * 1000
  "weighted_fairness_avg": 3000,    # 3.0 * 1000
  "vote_count": 1,
  "total_weight": [staker's staked amount],
  "is_reset": false,
  "last_reset_at": 0
}
```

**Checklist:**
- [ ] Aggregate retrieved successfully
- [ ] Weighted averages calculated correctly
- [ ] Vote count is 1
- [ ] Total weight matches staker's balance

#### Test 4.6: Test Duplicate Vote Rejection
```bash
soroban contract invoke \
  --network testnet \
  --source-account $STAKER_ADDRESS \
  --id $PUZZLE_VOTING_CONTRACT_ID \
  -- cast_vote \
  --voter $STAKER_ADDRESS \
  --puzzle_id $PUZZLE_ID \
  --difficulty 2 \
  --fun 3 \
  --fairness 2
```

**Expected:** Error with message "Voter has already voted on this puzzle"

**Checklist:**
- [ ] Duplicate vote correctly rejected
- [ ] Clear error message received

#### Test 4.7: Test Score Validation
```bash
soroban contract invoke \
  --network testnet \
  --source-account $STAKER_ADDRESS \
  --id $PUZZLE_VOTING_CONTRACT_ID \
  -- cast_vote \
  --voter $STAKER_ADDRESS \
  --puzzle_id 2 \
  --difficulty 0 \
  --fun 4 \
  --fairness 3
```

**Expected:** Error with message "Difficulty score must be between 1 and 5"

**Checklist:**
- [ ] Invalid score rejected
- [ ] Clear error message received
- [ ] Can vote with different puzzle_id (score 0 test)

### Step 5: Admin Function Testing

#### Test 5.1: Update Minimum Stake Threshold
```bash
NEW_THRESHOLD="50000000"  # 0.5 tokens

soroban contract invoke \
  --network testnet \
  --source-account $ADMIN_KEY \
  --id $PUZZLE_VOTING_CONTRACT_ID \
  -- update_min_stake_threshold \
  --new_threshold $NEW_THRESHOLD
```

**Checklist:**
- [ ] Threshold update succeeds
- [ ] Only admin can update (test with non-admin account should fail)
- [ ] New threshold takes effect for next votes

#### Test 5.2: Reset Puzzle Votes
```bash
soroban contract invoke \
  --network testnet \
  --source-account $ADMIN_KEY \
  --id $PUZZLE_VOTING_CONTRACT_ID \
  -- reset_puzzle_votes \
  --puzzle_id $PUZZLE_ID
```

**Checklist:**
- [ ] Reset succeeds
- [ ] Only admin can reset (verify)
- [ ] Aggregate shows reset flag after reset

#### Test 5.3: Verify Reset
```bash
soroban contract invoke \
  --network testnet \
  --id $PUZZLE_VOTING_CONTRACT_ID \
  -- get_aggregate \
  --puzzle_id $PUZZLE_ID
```

**Expected:** Aggregate with is_reset=true and vote_count=0

**Checklist:**
- [ ] Aggregate shows reset
- [ ] Vote count is 0
- [ ] last_reset_at is set
- [ ] Can vote again on same puzzle

### Step 6: Multi-Vote Scenario

#### Setup Multiple Voters
Repeat with different staker accounts to get multiple votes

#### Test Weighted Average with Multiple Voters
```bash
# Voter 1 (weight=2000): difficulty=2
# Voter 2 (weight=1000): difficulty=5
# Expected: (2*2000 + 5*1000) / 3000 = 3.0
```

**Checklist:**
- [ ] Multiple votes accepted
- [ ] Aggregate reflects all votes
- [ ] Weighted average is correct
- [ ] Precision scaling works (÷1000)

## Post-Deployment

### Documentation Updates
- [ ] Update CONTRACT_IDs in all documentation
- [ ] Record testnet deployment details
- [ ] Document any deviations from expected behavior
- [ ] Update integration guide with real addresses

### Monitoring
- [ ] Set up event monitoring for VoteCast events
- [ ] Set up event monitoring for VotesReset events
- [ ] Monitor contract storage usage
- [ ] Track transaction costs

### Backup & Record-Keeping
- [ ] Record contract address
- [ ] Record initialization parameters
- [ ] Backup Cargo.lock
- [ ] Tag git commit with deployment version
- [ ] Document any issues encountered

### Security Review
- [ ] Verify admin account security
- [ ] Confirm minimum stake threshold is appropriate
- [ ] Check staking contract integration works
- [ ] Verify event emission in testnet logs

## Staging (Before Mainnet)

### Final Testing
- [ ] Run all acceptance criteria tests
- [ ] Test with production-like data volume
- [ ] Test with multiple concurrent votes
- [ ] Performance test under load
- [ ] Gas cost profiling

### Code Audit
- [ ] Security audit of weighted average calculation
- [ ] Review storage efficiency
- [ ] Verify all error paths
- [ ] Check for integer overflow/underflow
- [ ] Verify cross-contract call safety

### Documentation Review
- [ ] All docs are current
- [ ] Integration guide reflects testnet experience
- [ ] Deployment guide is accurate
- [ ] No placeholder addresses remain

## Mainnet Deployment (Future)

When ready to move to mainnet:

1. [ ] Schedule mainnet deployment
2. [ ] Create production admin account (use institutional wallet)
3. [ ] Ensure staking contract is on mainnet
4. [ ] Deploy contract to mainnet
5. [ ] Initialize with mainnet parameters
6. [ ] Run full test suite on mainnet
7. [ ] Announce deployment to users
8. [ ] Monitor closely for first 24 hours

## Rollback Procedure

If issues are found:

1. [ ] Stop promoting voting to users
2. [ ] Reset puzzle votes if needed
3. [ ] Document issues
4. [ ] Fix code locally
5. [ ] Redeploy as new contract
6. [ ] Migrate votes if critical (requires custom script)

## Sign-Off

```
Deployment By: ________________________  Date: ____________
Approved By:   ________________________  Date: ____________
Tested By:     ________________________  Date: ____________

Contract ID Testnet: _______________________________________
Contract ID Mainnet: _______________________________________

Issues Found: [ ] None  [ ] Minor  [ ] Major
If Major, describe: ________________________________
Resolution: ________________________________________
```

---

**Last Updated**: March 30, 2026
**Checklist Version**: 1.0
