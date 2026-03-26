# Gamification Rewards Integration Guide

## Overview

The Gamification Rewards contract provides a comprehensive multiplier system that can be integrated with any game or reward distribution system in the Stellar ecosystem.

## Architecture

```
┌─────────────────┐
│  Your Game      │
│  Contract       │
└────────┬────────┘
         │
         │ Calls
         ↓
┌─────────────────────────────────────┐
│  Gamification Rewards Contract      │
│  ┌──────────────┐  ┌──────────────┐ │
│  │ Streak       │  │ Combo        │ │
│  │ Tracker      │  │ System       │ │
│  └──────────────┘  └──────────────┘ │
│  ┌──────────────┐  ┌──────────────┐ │
│  │ Milestone    │  │ Boost        │ │
│  │ System       │  │ Manager      │ │
│  └──────────────┘  └──────────────┘ │
│  ┌──────────────┐  ┌──────────────┐ │
│  │ Multiplier   │  │ Leaderboard  │ │
│  │ Calculator   │  │ System       │ │
│  └──────────────┘  └──────────────┘ │
└─────────────────────────────────────┘
         │
         │ Returns Multiplied Rewards
         ↓
┌─────────────────┐
│  Token Transfer │
│  to Player      │
└─────────────────┘
```

## Integration Steps

### Step 1: Deploy and Initialize

```bash
# Deploy the contract
./deploy_gamification_rewards.sh

# Initialize with admin
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- initialize \
  --admin <ADMIN_ADDRESS>
```

### Step 2: Configure Parameters (Optional)

```bash
# Set custom milestone thresholds
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- set_milestone_threshold \
  --admin <ADMIN_ADDRESS> \
  --level 1 \
  --threshold 25

# Configure combo decay
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- update_combo_decay \
  --admin <ADMIN_ADDRESS> \
  --combo-decay-period 43200 \
  --combo-decay-rate 1
```

### Step 3: Integrate with Your Contract

#### Rust Integration Example

```rust
// In your game contract's Cargo.toml
[dependencies]
gamification_rewards = { path = "../gamification_rewards" }
soroban-sdk = "21.0.0"

// In your contract code
use soroban_sdk::{contract, contractimpl, Address, Env};
use gamification_rewards::GamificationRewardsContractClient;

#[contract]
pub struct YourGameContract;

#[contractimpl]
impl YourGameContract {
    // Store the multiplier contract address
    pub fn set_multiplier_contract(
        env: Env, 
        admin: Address, 
        multiplier_contract: Address
    ) {
        admin.require_auth();
        env.storage().persistent().set(
            &DataKey::MultiplierContract, 
            &multiplier_contract
        );
    }
    
    // Complete a quest and calculate rewards
    pub fn complete_quest(env: Env, player: Address, quest_id: u32) -> u128 {
        player.require_auth();
        
        // Get multiplier contract client
        let multiplier_contract: Address = env
            .storage()
            .persistent()
            .get(&DataKey::MultiplierContract)
            .expect("Multiplier contract not set");
            
        let multiplier_client = GamificationRewardsContractClient::new(
            &env,
            &multiplier_contract
        );
        
        // Record player actions for multipliers
        multiplier_client.record_daily_action(&player);
        multiplier_client.record_combo_action(&player);
        
        // Calculate base quest reward
        let base_reward = Self::calculate_base_quest_reward(&quest_id);
        
        // Apply multipliers
        let final_reward = multiplier_client.calculate_reward(
            &player, 
            &base_reward
        );
        
        // Update leaderboard
        multiplier_client.update_leaderboard(&player);
        
        // Transfer tokens to player
        Self::transfer_reward(&player, final_reward);
        
        final_reward
    }
    
    fn calculate_base_quest_reward(quest_id: &u32) -> u128 {
        // Your quest reward logic here
        1000 // Base reward
    }
    
    fn transfer_reward(player: &Address, amount: u128) {
        // Your token transfer logic here
    }
}
```

### Step 4: Add Admin Functions

```rust
// Activate boost for a player (e.g., during special events)
pub fn activate_event_boost(
    env: Env,
    admin: Address,
    player: Address,
    boost_type: BoostType,
    duration: u32,
) {
    admin.require_auth();
    
    let multiplier_contract: Address = env
        .storage()
        .persistent()
        .get(&DataKey::MultiplierContract)
        .expect("Multiplier contract not set");
        
    let multiplier_client = GamificationRewardsContractClient::new(
        &env,
        &multiplier_contract
    );
    
    multiplier_client.activate_boost(
        &admin,
        &player,
        &boost_type,
        &duration
    );
}

// View player's current multiplier
pub fn get_player_multiplier(env: Env, player: Address) -> MultiplierState {
    let multiplier_contract: Address = env
        .storage()
        .persistent()
        .get(&DataKey::MultiplierContract)
        .expect("Multiplier contract not set");
        
    let multiplier_client = GamificationRewardsContractClient::new(
        &env,
        &multiplier_contract
    );
    
    multiplier_client.get_total_multiplier(&player)
}
```

## Use Cases

### 1. Daily Login Bonus System

```rust
pub fn claim_daily_bonus(env: Env, player: Address) -> u128 {
    player.require_auth();
    
    let multiplier_client = Self::get_multiplier_client(&env);
    
    // Record daily action
    multiplier_client.record_daily_action(&player);
    
    // Calculate base bonus
    let base_bonus = 500;
    
    // Apply multipliers
    let total_bonus = multiplier_client.calculate_reward(
        &player,
        &base_bonus
    );
    
    // Transfer bonus
    Self::transfer_tokens(&player, total_bonus);
    
    total_bonus
}
```

### 2. Quest Chain Completion

```rust
pub fn complete_quest_chain(
    env: Env,
    player: Address,
    quest_ids: Vec<u32>,
) -> u128 {
    player.require_auth();
    
    let multiplier_client = Self::get_multiplier_client(&env);
    
    // Record combo actions for each quest
    for _ in 0..quest_ids.len() {
        multiplier_client.record_combo_action(&player);
    }
    
    // Base chain reward
    let chain_length = quest_ids.len();
    let base_reward = 1000 * chain_length as u128;
    
    // Apply multipliers
    let total_reward = multiplier_client.calculate_reward(
        &player,
        &base_reward
    );
    
    // Update leaderboard
    multiplier_client.update_leaderboard(&player);
    
    Self::transfer_tokens(&player, total_reward);
    
    total_reward
}
```

### 3. Achievement Unlocks

```rust
pub fn unlock_achievement(
    env: Env,
    player: Address,
    achievement_id: u32,
) {
    let admin = Self::get_admin(&env);
    let multiplier_client = Self::get_multiplier_client(&env);
    
    // Record milestone action
    multiplier_client.record_milestone_action(&admin, &player);
    
    // Grant achievement-specific rewards
    let achievement_reward = Self::get_achievement_reward(achievement_id);
    
    // Apply multipliers
    let total_reward = multiplier_client.calculate_reward(
        &player,
        &achievement_reward
    );
    
    Self::transfer_tokens(&player, total_reward);
}
```

### 4. Tournament Rewards

```rust
pub fn distribute_tournament_rewards(
    env: Env,
    winners: Vec<(Address, u32)>, // (player, rank)
) {
    let admin = Self::get_admin(&env);
    let multiplier_client = Self::get_multiplier_client(&env);
    
    for (player, rank) in winners.iter() {
        // Base reward based on rank
        let base_reward = match rank {
            1 => 10000,
            2 => 5000,
            3 => 2500,
            _ => 1000,
        };
        
        // Apply multipliers
        let total_reward = multiplier_client.calculate_reward(
            &player,
            &base_reward
        );
        
        // Update leaderboard
        multiplier_client.update_leaderboard(&player);
        
        // Special boost for winner
        if rank == 1 {
            multiplier_client.activate_boost(
                &admin,
                &player,
                &BoostType::SuperBoost,
                &DAY_IN_LEDGERS
            );
        }
        
        Self::transfer_tokens(&player, total_reward);
    }
}
```

### 5. Event Participation

```rust
pub fn participate_in_event(
    env: Env,
    player: Address,
    event_score: u32,
) {
    player.require_auth();
    
    let multiplier_client = Self::get_multiplier_client(&env);
    
    // Record combo based on performance
    let combo_count = event_score / 100;
    for _ in 0..combo_count {
        multiplier_client.record_combo_action(&player);
    }
    
    // Base participation reward
    let base_reward = 200;
    
    // Apply multipliers
    let total_reward = multiplier_client.calculate_reward(
        &player,
        &base_reward
    );
    
    Self::transfer_tokens(&player, total_reward);
}
```

## Frontend Integration

### JavaScript/TypeScript Example

```typescript
import { SorobanClient } from 'soroban-client';

const CONTRACT_ID = 'CDJ...'; // Your contract ID
const server = new SorobanClient('https://soroban-test.stellar.org');

// Record daily action
async function recordDailyAction(playerAddress: string) {
    const contract = await server.loadContract(CONTRACT_ID);
    
    const tx = await server.createTransaction({
        source: playerAddress,
        operations: [{
            type: 'invokeHostFunction',
            func: {
                type: 'InvokeContractFn',
                contractAddress: CONTRACT_ID,
                function: 'record_daily_action',
                args: [playerAddress]
            }
        }]
    });
    
    return await server.sendTransaction(tx);
}

// Get player's current multiplier
async function getPlayerMultiplier(playerAddress: string) {
    const contract = await server.loadContract(CONTRACT_ID);
    
    const result = await contract.get_total_multiplier(playerAddress);
    
    return {
        baseMultiplier: result.base_multiplier / 100,
        streakMultiplier: result.streak_multiplier / 100,
        comboMultiplier: result.combo_multiplier / 100,
        milestoneMultiplier: result.milestone_multiplier / 100,
        boostMultiplier: result.boost_multiplier / 100,
        totalMultiplier: result.total_multiplier / 100
    };
}

// Calculate reward with multipliers
async function calculateReward(playerAddress: string, baseReward: bigint) {
    const contract = await server.loadContract(CONTRACT_ID);
    
    const finalReward = await contract.calculate_reward(
        playerAddress,
        baseReward
    );
    
    return finalReward;
}

// Get leaderboard
async function getLeaderboard(limit: number = 10) {
    const contract = await server.loadContract(CONTRACT_ID);
    
    const entries = await contract.get_leaderboard(limit);
    
    return entries.map(entry => ({
        player: entry.player,
        totalMultiplier: entry.total_multiplier / 100,
        totalActions: entry.total_actions,
        bestCombo: entry.best_combo,
        bestStreak: entry.best_streak,
        rank: entries.indexOf(entry) + 1
    }));
}

// Check player rank
async function getPlayerRank(playerAddress: string) {
    const contract = await server.loadContract(CONTRACT_ID);
    
    const rank = await contract.get_player_rank(playerAddress);
    
    return rank;
}
```

### React Component Example

```tsx
import React, { useState, useEffect } from 'react';

interface MultiplierState {
    totalMultiplier: number;
    streakMultiplier: number;
    comboMultiplier: number;
    milestoneMultiplier: number;
    boostMultiplier: number;
}

export function PlayerMultipliers({ playerAddress }: { playerAddress: string }) {
    const [multipliers, setMultipliers] = useState<MultiplierState | null>(null);
    const [rank, setRank] = useState<number>(0);

    useEffect(() => {
        async function fetchMultipliers() {
            const state = await getPlayerMultiplier(playerAddress);
            setMultipliers(state);
            
            const playerRank = await getPlayerRank(playerAddress);
            setRank(playerRank);
        }
        
        fetchMultipliers();
    }, [playerAddress]);

    if (!multipliers) return <div>Loading...</div>;

    return (
        <div className="multiplier-card">
            <h3>Your Multipliers</h3>
            
            <div className="multiplier-stat">
                <span className="label">Total Multiplier:</span>
                <span className="value">{multipliers.totalMultiplier.toFixed(2)}x</span>
            </div>
            
            <div className="multiplier-breakdown">
                <div>
                    <span>Streak:</span>
                    <span>{multipliers.streakMultiplier.toFixed(2)}x</span>
                </div>
                <div>
                    <span>Combo:</span>
                    <span>{multipliers.comboMultiplier.toFixed(2)}x</span>
                </div>
                <div>
                    <span>Milestone:</span>
                    <span>{multipliers.milestoneMultiplier.toFixed(2)}x</span>
                </div>
                <div>
                    <span>Boosts:</span>
                    <span>{multipliers.boostMultiplier.toFixed(2)}x</span>
                </div>
            </div>
            
            {rank > 0 && (
                <div className="leaderboard-rank">
                    🏆 Rank #{rank}
                </div>
            )}
        </div>
    );
}

export function Leaderboard() {
    const [entries, setEntries] = useState<any[]>([]);

    useEffect(() => {
        async function fetchLeaderboard() {
            const data = await getLeaderboard(10);
            setEntries(data);
        }
        
        fetchLeaderboard();
    }, []);

    return (
        <div className="leaderboard">
            <h3>Top Players</h3>
            <table>
                <thead>
                    <tr>
                        <th>Rank</th>
                        <th>Player</th>
                        <th>Multiplier</th>
                        <th>Best Combo</th>
                        <th>Best Streak</th>
                    </tr>
                </thead>
                <tbody>
                    {entries.map((entry, index) => (
                        <tr key={entry.player}>
                            <td>#{index + 1}</td>
                            <td>{entry.player.slice(0, 6)}...{entry.player.slice(-4)}</td>
                            <td>{entry.totalMultiplier.toFixed(2)}x</td>
                            <td>{entry.bestCombo}</td>
                            <td>{entry.bestStreak}</td>
                        </tr>
                    ))}
                </tbody>
            </table>
        </div>
    );
}
```

## Best Practices

### 1. Gas Optimization

- Batch multiple actions together when possible
- Update leaderboard periodically, not after every action
- Limit history queries to necessary entries only

### 2. User Experience

- Show multiplier previews before claiming rewards
- Display progress bars for streaks and combos
- Send notifications when streaks are about to expire
- Highlight milestone achievements

### 3. Balance Tuning

Start with conservative values and adjust based on gameplay:

```rust
// Initial conservative settings
MAX_STREAK_MULTIPLIER: 300  // 3x max
COMBO_DECAY_PERIOD: DAY_IN_LEDGERS  // 1 day
MILESTONE_BONUS: 5  // 5% per level

// After balancing, increase if needed
MAX_STREAK_MULTIPLIER: 500  // 5x max
COMBO_DECAY_PERIOD: DAY_IN_LEDGERS / 2  // 12 hours
MILESTONE_BONUS: 10  // 10% per level
```

### 4. Security

- Always verify admin addresses
- Implement rate limiting for admin functions
- Monitor for unusual multiplier patterns
- Keep emergency pause functionality ready

## Troubleshooting

### Issue: Multipliers not applying correctly

**Solution**: Verify all action recording calls are made before reward calculation:

```rust
// ✅ Correct order
multiplier_client.record_daily_action(&player);
multiplier_client.record_combo_action(&player);
let reward = multiplier_client.calculate_reward(&player, &base);

// ❌ Wrong order
let reward = multiplier_client.calculate_reward(&player, &base);
multiplier_client.record_daily_action(&player); // Too late!
```

### Issue: Combo decaying too fast

**Solution**: Adjust decay parameters:

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- update_combo_decay \
  --admin <ADMIN> \
  --combo-decay-period 172800 \
  --combo-decay-rate 0
```

### Issue: Leaderboard not updating

**Solution**: Ensure `update_leaderboard()` is called explicitly:

```rust
// Must call this to update rankings
multiplier_client.update_leaderboard(&player);
```

## Performance Metrics

Expected costs on Stellar testnet:

- `record_daily_action`: ~0.001 XLM
- `record_combo_action`: ~0.001 XLM
- `calculate_reward`: ~0.002 XLM
- `update_leaderboard`: ~0.003 XLM
- `activate_boost`: ~0.002 XLM

## Support

For issues or questions:
- Check the README.md for detailed API documentation
- Review test cases for usage examples
- Consult the Stellar Discord developer channel

---

**Version**: 1.0.0
**Last Updated**: March 25, 2026
