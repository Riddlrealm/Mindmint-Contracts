# Gamification Rewards - Quick Reference Card

## 🚀 Quick Start (3 Steps)

```bash
# 1. Deploy
./deploy_gamification_rewards.sh

# 2. Initialize
soroban contract invoke --id <ID> -- initialize --admin <ADMIN>

# 3. Use
soroban contract invoke --id <ID> -- record_daily_action --player <PLAYER>
```

---

## 📊 Multiplier Breakdown

| Type | Formula | Max |
|------|---------|-----|
| **Daily Streak** | +5% per day | +150% (30 days) |
| **Weekly Streak** | +10% per week | +120% (12 weeks) |
| **Combo Chain** | +2% per combo | +200% (100 combos) |
| **Milestones** | +5% per level | Unlimited |
| **Boosts** | Varies by type | +100% (Super) |

**Total Formula**: `Base + (Sum of Bonuses / 4)`, capped at 5x

---

## 🔧 Common Commands

### Player Actions
```bash
# Record daily action
soroban contract invoke --id <ID> -- record_daily_action --player <PLAYER>

# Record combo action  
soroban contract invoke --id <ID> -- record_combo_action --player <PLAYER>

# Get current multiplier
soroban contract invoke --id <ID> -- get_total_multiplier --player <PLAYER>

# Calculate reward
soroban contract invoke --id <ID> -- calculate_reward --player <PLAYER> --base-reward 1000
```

### View Functions
```bash
# Get streak data
soroban contract invoke --id <ID> -- get_streak_data --player <PLAYER>

# Get combo data
soroban contract invoke --id <ID> -- get_combo_data --player <PLAYER>

# Get leaderboard (top 10)
soroban contract invoke --id <ID> -- get_leaderboard --limit 10

# Get player rank
soroban contract invoke --id <ID> -- get_player_rank --player <PLAYER>
```

### Admin Functions
```bash
# Activate boost for player
soroban contract invoke --id <ID> \
  -- activate_boost \
  --admin <ADMIN> --player <PLAYER> \
  --boost-type SpeedBoost --duration-ledgers 17280

# Set milestone threshold
soroban contract invoke --id <ID> \
  -- set_milestone_threshold \
  --admin <ADMIN> --level 1 --threshold 25

# Pause contract
soroban contract invoke --id <ID> \
  -- set_paused \
  --admin <ADMIN> --paused true
```

---

## 💻 Code Snippets

### Rust Integration
```rust
// Initialize client
let client = GamificationRewardsContractClient::new(&env, &CONTRACT_ID);

// Record actions
client.record_daily_action(&player);
client.record_combo_action(&player);

// Calculate reward with multipliers
let final_reward = client.calculate_reward(&player, &base_reward);

// Update leaderboard
client.update_leaderboard(&player);
```

### TypeScript Integration
```typescript
const contract = server.loadContract(CONTRACT_ID);

// Record action
await contract.record_daily_action(playerAddress);

// Get multiplier
const mult = await contract.get_total_multiplier(playerAddress);
console.log(`Multiplier: ${mult.total_multiplier / 100}x`);

// Calculate reward
const reward = await contract.calculate_reward(playerAddress, 1000n);
```

---

## 🎯 Boost Types

| Type | Bonus | Duration | Best For |
|------|-------|----------|----------|
| **SpeedBoost** | +50% | Short (1-3 days) | Quick events |
| **LuckBoost** | +30% | Medium (1 week) | Regular play |
| **PowerBoost** | +20% | Long (2+ weeks) | Long-term |
| **SuperBoost** | +100% | Very short (1 day) | Special rewards |

---

## 📈 Example Progression

**Day 1**: New player
- Streak: 1 day → +5%
- Combo: 1 → +2%
- **Total**: 1.07x

**Week 1**: Dedicated player
- Streak: 7 days → +35%
- Combo: 10 → +20%
- **Total**: 1.55x

**Month 1**: Veteran player
- Streak: 30 days → +150% (capped)
- Combo: 50 → +100%
- Milestone: 3 levels → +15%
- **Total**: 2.66x

**With Boost**: Event participant
- All above + Speed Boost → +50%
- **Total**: 3.16x

---

## ⚙️ Configuration

### Default Values
```
DAY_IN_LEDGERS = 17280 (~24h)
WEEK_IN_LEDGERS = 120960 (7 days)
MAX_COMBO = 100
MAX_MULTIPLIER = 500 (5x)
COMBO_DECAY_START = 1 day
COMBO_DECAY_RATE = 1/ledger
```

### Tune for Your Game
```bash
# Faster decay (more intense)
soroban contract invoke --id <ID> \
  -- update_combo_decay \
  --admin <ADMIN> \
  --combo-decay-period 43200 \
  --combo-decay-rate 2

# Slower decay (more casual)
soroban contract invoke --id <ID> \
  -- update_combo_decay \
  --admin <ADMIN> \
  --combo-decay-period 259200 \
  --combo-decay-rate 0
```

---

## 🏆 Leaderboard Examples

```bash
# Get top 10
soroban contract invoke --id <ID> -- get_leaderboard --limit 10

# Get your rank
soroban contract invoke --id <ID> -- get_player_rank --player <YOU>

# Update after big win
soroban contract invoke --id <ID> -- update_leaderboard --player <YOU>
```

---

## 🧪 Testing

```bash
# Run all tests
cargo test -p gamification_rewards

# Run specific test
cargo test -p gamification_rewards test_daily_streak_increases

# Test with output
cargo test -p gamification_rewards -- --nocapture
```

---

## 🔍 Debugging

### Check Contract State
```bash
# Get config
soroban contract invoke --id <ID> -- get_config

# Get global stats
soroban contract invoke --id <ID> -- get_global_stats

# Get player history (last 10)
soroban contract invoke --id <ID> \
  -- get_multiplier_history \
  --player <PLAYER> --limit 10
```

### Common Issues

**Issue**: Multiplier seems low
- **Check**: Are you recording actions before calculating reward?
- **Fix**: Call `record_*_action` first, then `calculate_reward`

**Issue**: Combo decayed to 0
- **Check**: How long since last action?
- **Fix**: Adjust decay period or play more frequently

**Issue**: Boost not applying
- **Check**: Is boost still active?
- **Fix**: Check duration and current ledger

---

## 📞 Resources

- **Full Docs**: `README.md`
- **Integration Guide**: `INTEGRATION_GUIDE.md`
- **Implementation Details**: `IMPLEMENTATION_SUMMARY.md`
- **Deployment Script**: `deploy_gamification_rewards.sh`

---

## 🎮 Complete Example Flow

```bash
# Daily login routine
PLAYER=<YOUR_ADDRESS>
CONTRACT=<CONTRACT_ID>

# 1. Record daily login
soroban contract invoke --id $CONTRACT -- record_daily_action --player $PLAYER

# 2. Do some quests (record combos)
for i in {1..5}; do
  soroban contract invoke --id $CONTRACT -- record_combo_action --player $PLAYER
done

# 3. Check your multiplier
soroban contract invoke --id $CONTRACT -- get_total_multiplier --player $PLAYER

# 4. Claim quest reward with multiplier
soroban contract invoke --id $CONTRACT \
  -- calculate_reward \
  --player $PLAYER \
  --base-reward 1000

# 5. Update leaderboard position
soroban contract invoke --id $CONTRACT -- update_leaderboard --player $PLAYER

# 6. Check new rank
soroban contract invoke --id $CONTRACT -- get_player_rank --player $PLAYER
```

---

**Quick Reference v1.0** | March 25, 2026 | Part of Quest Contract Ecosystem
