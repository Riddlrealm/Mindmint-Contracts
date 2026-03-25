# Gamification Rewards & Multiplier Contract

A comprehensive Soroban smart contract for managing player rewards multipliers based on streaks, combo chains, achievements, and temporary boosts.

## 🎯 Features

### Core Mechanics

1. **Streak-Based Multipliers**
   - Daily streak tracking with increasing bonuses
   - Weekly streak tracking for long-term engagement
   - Grace periods to maintain streaks
   - Best streak records for leaderboard calculations

2. **Combo Chain System**
   - Track consecutive actions within time windows
   - Combo decay after inactivity periods
   - Configurable decay rates and periods
   - Best combo record tracking

3. **Milestone Unlocks**
   - Permanent multiplier bonuses for reaching action milestones
   - Configurable milestone thresholds (default: 10, 50, 100, 250, 500, 1000 actions)
   - Progressive bonus scaling (5% per milestone level)

4. **Temporary Boost Items**
   - Speed Boost: Short duration, high multiplier (+0.5x)
   - Luck Boost: Medium duration, medium multiplier (+0.3x)
   - Power Boost: Long duration, low multiplier (+0.2x)
   - Super Boost: Very short duration, very high multiplier (+1.0x)

5. **Multiplier Stacking Rules**
   - Additive bonuses within categories
   - Simplified multiplicative stacking across categories
   - Maximum cap at 5x total multiplier

6. **Expiration Logic**
   - Automatic boost expiration based on ledger numbers
   - Combo decay over time
   - History tracking with automatic pruning (last 50 entries)

7. **History Tracking**
   - Complete multiplier history per player
   - Breakdown by multiplier type
   - Ledger timestamp for each entry

8. **Leaderboard System**
   - Global rankings by total multiplier
   - Top N player listings
   - Player rank queries
   - Automatic sorting and updating

## 📊 Multiplier Calculation

### Formula

```
Total Multiplier = Base + (Sum of Bonuses / 4), capped at 5x
```

Where:
- Base Multiplier = 100 (1x)
- Streak Bonus = (Daily Streak × 5%) + (Weekly Streak × 10%), max 2.7x
- Combo Bonus = (Combo Count × 2%), max 2x
- Milestone Bonus = Permanent bonus from unlocks, starts at 5%
- Boost Bonus = Sum of active boost items, varies by type

### Example Calculations

**Scenario 1: Casual Player**
- 7-day daily streak: +35%
- 1-week weekly streak: +10%
- 5-combo chain: +10%
- No milestones or boosts
- **Total Multiplier**: 1.0 + (0.35 + 0.10 + 0.10) / 4 = **1.14x**

**Scenario 2: Dedicated Player**
- 30-day daily streak: +150% (capped)
- 12-week weekly streak: +120% (capped)
- 50-combo chain: +100%
- 3 milestones unlocked: +15%
- Speed Boost active: +50%
- **Total Multiplier**: 1.0 + (1.5 + 1.2 + 1.0 + 0.15 + 0.5) / 4 = **2.09x**

## 🚀 Quick Start

### Prerequisites

- Rust toolchain installed
- Soroban CLI 21.0.0+
- Stellar testnet account

### Build

```bash
cd contracts/gamification_rewards
cargo build --target wasm32-unknown-unknown --release
```

### Test

```bash
cargo test
```

### Deploy to Testnet

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/gamification_rewards.wasm \
  --source <YOUR_SOURCE> \
  --network testnet
```

## 📖 API Reference

### Initialization

#### `initialize(env: Env, admin: Address)`
Initialize the contract with an admin address.

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- initialize \
  --admin <ADMIN_ADDRESS>
```

### Admin Functions

#### `add_verifier(env: Env, admin: Address, verifier: Address)`
Add an authorized score verifier.

#### `remove_verifier(env: Env, admin: Address, verifier: Address)`
Remove an authorized verifier.

#### `set_paused(env: Env, admin: Address, paused: bool)`
Pause or unpause the contract.

#### `update_combo_decay(env: Env, admin: Address, combo_decay_period: u32, combo_decay_rate: u32)`
Update combo decay parameters.

#### `set_milestone_threshold(env: Env, admin: Address, level: u32, threshold: u32)`
Set milestone threshold for a specific level.

#### `activate_boost(env: Env, admin: Address, player: Address, boost_type: BoostType, duration_ledgers: u32)`
Activate a boost item for a player.

### Player Functions

#### `record_daily_action(env: Env, player: Address) -> u32`
Record a daily action and update streak. Returns current streak multiplier.

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- record_daily_action \
  --player <PLAYER_ADDRESS>
```

#### `record_combo_action(env: Env, player: Address) -> u32`
Record a combo action. Returns current combo multiplier.

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- record_combo_action \
  --player <PLAYER_ADDRESS>
```

#### `record_milestone_action(env: Env, player: Address) -> u32`
Record a milestone action (admin only). Returns milestone multiplier.

#### `get_total_multiplier(env: Env, player: Address) -> MultiplierState`
Get the complete multiplier breakdown for a player.

#### `calculate_reward(env: Env, player: Address, base_reward: u128) -> u128`
Calculate final reward with all multipliers applied.

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- calculate_reward \
  --player <PLAYER_ADDRESS> \
  --base-reward 1000
```

### View Functions

#### `get_streak_data(env: Env, player: Address) -> StreakData`
Get current streak data for a player.

#### `get_combo_data(env: Env, player: Address) -> ComboChain`
Get current combo chain data.

#### `get_milestone_progress(env: Env, player: Address) -> MilestoneProgress`
Get milestone progress for a player.

#### `get_active_boosts(env: Env, player: Address) -> Vec<BoostItem>`
Get active boosts for a player.

#### `get_multiplier_history(env: Env, player: Address, limit: u32) -> Vec<MultiplierHistoryEntry>`
Get multiplier history (limited to last 50 entries).

#### `get_leaderboard(env: Env, limit: u32) -> Vec<PlayerLeaderboardEntry>`
Get top players by multiplier.

#### `get_player_rank(env: Env, player: Address) -> u32`
Get player's current rank on leaderboard.

#### `get_global_stats(env: Env) -> GlobalStats`
Get global statistics (total players, highest combo, etc.).

#### `get_config(env: Env) -> Config`
Get current contract configuration.

## 🏆 Leaderboard

The leaderboard system tracks top players based on their total multiplier scores. Features include:

- **Automatic Updates**: Call `update_leaderboard()` to refresh player standings
- **Configurable Size**: Max entries set during initialization (default: 100)
- **Sorted Ranking**: Players sorted by total multiplier in descending order
- **Rank Tracking**: Query individual player ranks instantly

### Example Usage

```bash
# Update leaderboard after player actions
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- update_leaderboard \
  --player <PLAYER_ADDRESS>

# Get top 10 players
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- get_leaderboard \
  --limit 10

# Check player rank
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- get_player_rank \
  --player <PLAYER_ADDRESS>
```

## ⚙️ Configuration

Default configuration values:

```rust
DAY_IN_LEDGERS: 17280          // ~24 hours
WEEK_IN_LEDGERS: 120960        // 7 days
MAX_COMBO_CHAIN: 100           // Maximum combo points
MAX_STREAK_MULTIPLIER: 500     // 5x cap
BASE_MULTIPLIER: 100           // 1x base
COMBO_DECAY_PERIOD: 1 day      // Before decay starts
COMBO_DECAY_RATE: 1            // Per ledger after decay starts
```

## 🧪 Testing

Comprehensive tests cover:

- ✅ Initialization
- ✅ Daily streak increases
- ✅ Combo chain stacking
- ✅ Milestone unlocks and permanent bonuses
- ✅ Boost item activation and expiration
- ✅ Multiplier calculation accuracy
- ✅ Reward calculation with multipliers
- ✅ Leaderboard updates and ranking
- ✅ Multiplier history tracking
- ✅ Combo decay mechanics
- ✅ Global statistics tracking

Run tests:

```bash
cargo test --package gamification_rewards
```

## 📈 Integration Example

```rust
// In your game contract, import and use the multiplier contract

use gamification_rewards::GamificationRewardsContractClient;

// Initialize client
let multiplier_client = GamificationRewardsContractClient::new(
    &env,
    &MULTIPLIER_CONTRACT_ID
);

// Record player action
multiplier_client.record_daily_action(&player);
multiplier_client.record_combo_action(&player);

// Calculate reward with multipliers
let base_reward = 1000;
let final_reward = multiplier_client.calculate_reward(&player, &base_reward);

// Update leaderboard
multiplier_client.update_leaderboard(&player);

// Transfer final reward to player
token_client.transfer(&reward_pool, &player, &final_reward);
```

## 🔒 Security Considerations

- **Authentication**: All player actions require `require_auth()`
- **Admin Controls**: Sensitive functions restricted to admin
- **Verifier System**: Authorized verifiers can record milestone actions
- **Pause Mechanism**: Emergency pause functionality
- **Storage Limits**: History pruned to last 50 entries to prevent bloat
- **Caps and Limits**: Maximum multipliers and combo chains enforced

## 🎮 Use Cases

1. **Daily Login Rewards**: Increase rewards for consecutive daily logins
2. **Quest Completion**: Bonus multipliers for completing quest chains
3. **Achievement Systems**: Permanent bonuses for unlocking achievements
4. **Event Participation**: Temporary boosts during special events
5. **Skill-based Rewards**: Higher multipliers for better performance
6. **Social Features**: Leaderboards and competition

## 🛠️ Advanced Features

### Custom Milestone Configuration

```bash
# Set custom thresholds
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- set_milestone_threshold \
  --admin <ADMIN_ADDRESS> \
  --level 1 \
  --threshold 25

soroban contract invoke \
  --id <CONTRACT_ID> \
  -- set_milestone_threshold \
  --admin <ADMIN_ADDRESS> \
  --level 2 \
  --threshold 100
```

### Combo Decay Tuning

```bash
# Adjust decay parameters
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- update_combo_decay \
  --admin <ADMIN_ADDRESS> \
  --combo-decay-period 86400 \
  --combo-decay-rate 2
```

## 📝 Data Structures

### MultiplierState
```rust
pub struct MultiplierState {
    pub base_multiplier: u32,         // 100 = 1x
    pub streak_multiplier: u32,       // From streaks
    pub combo_multiplier: u32,        // From combos
    pub milestone_multiplier: u32,    // From milestones
    pub boost_multiplier: u32,        // From boosts
    pub total_multiplier: u32,        // Final calculated
}
```

### StreakData
```rust
pub struct StreakData {
    pub daily_streak: u32,
    pub weekly_streak: u32,
    pub last_daily_claim: u32,
    pub last_weekly_claim: u32,
    pub best_daily_streak: u32,
    pub best_weekly_streak: u32,
}
```

### ComboChain
```rust
pub struct ComboChain {
    pub current_combo: u32,
    pub best_combo: u32,
    pub last_action_ledger: u32,
    pub combo_decay_start: u32,
}
```

## 🌟 Events Emitted

- `multiplier_update`: When multiplier state changes
- `streak_achieved`: When player reaches new streak
- `combo_record`: When combo chain updates
- `milestone_unlock`: When milestone is unlocked
- `boost_activated`: When boost item activated
- `boost_expired`: When boost expires

## 📄 License

Part of the Quest Contract ecosystem.

## 🤝 Contributing

This contract follows the Soroban development standards and Stellar ecosystem best practices.

---

**Contract Address**: [Deploy to testnet first]
**Version**: 1.0.0
**Last Updated**: March 25, 2026
