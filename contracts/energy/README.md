# Energy and Stamina Management Contract

This Soroban contract manages player energy/stamina for the quest system with time-based regeneration, token-based refills, and boost mechanisms.

## Energy Economics

### Core Mechanics

- **Maximum Energy**: 100 energy units (configurable)
- **Base Regeneration**: 1 energy per second (configurable)
- **Puzzle Cost**: 10 energy units per attempt (configurable)
- **Refill Cost**: 50 reward tokens for full energy refill (configurable)

### Time-Based Regeneration

Energy regenerates continuously over time using the formula:
```
regenerated_energy = time_elapsed_seconds × base_regen_rate × boost_multiplier
```

Where:
- `time_elapsed_seconds`: Seconds since last energy update
- `base_regen_rate`: Configurable base regeneration rate (default: 1 energy/second)
- `boost_multiplier`: 1x, 2x, 3x, or 5x depending on active boost

### Boost System

Players can activate temporary regeneration boosts:

| Boost Type | Multiplier | Description |
|------------|------------|-------------|
| None | 1x | Normal regeneration |
| DoubleRegen | 2x | Double regeneration rate |
| TripleRegen | 3x | Triple regeneration rate |
| QuintupleRegen | 5x | Quintuple regeneration rate |

Boosts have configurable duration and only one boost can be active at a time.

### Energy Gifting

Players can gift energy to other players with these limits:
- **Daily Gift Limit**: 20 energy units per player per day
- **Gift Reset**: Daily at midnight UTC
- **Receiver Cap**: Cannot exceed maximum energy capacity

### Token-Based Refills

- **Cost**: 50 reward tokens for full energy refill
- **Instant**: Refills energy to maximum immediately
- **Token Transfer**: Tokens are transferred to contract treasury

## Contract Functions

### Initialization
```rust
initialize(admin, reward_token, base_regen_rate, default_max_energy, puzzle_energy_cost, refill_token_cost)
```

### Player Functions
- `consume_energy_for_puzzle(player)` - Consume energy for puzzle attempts
- `instant_refill(player)` - Refill energy using tokens
- `gift_energy(from_player, to_player, amount)` - Gift energy between players
- `apply_boost(player, boost_type, duration_seconds)` - Apply regeneration boost

### View Functions
- `get_current_energy(player)` - Get current energy (with regeneration applied)
- `get_player_energy_info(player)` - Get raw player energy data
- `get_config()` - Get contract configuration

### Admin Functions
- `update_config(...)` - Update contract parameters
- `set_paused(paused)` - Pause/unpause contract

## Storage Optimization

### Timestamp Calculations
- Uses ledger timestamps for time tracking
- Calculates regeneration on-demand to minimize storage writes
- Stores `last_update` timestamp per player
- Applies time-based calculations efficiently using integer arithmetic

### Data Structures
```rust
PlayerEnergy {
    current_energy: u32,      // Current energy amount
    max_energy: u32,          // Maximum energy capacity
    last_update: u64,         // Last update timestamp
    active_boost: BoostType,  // Current boost type
    boost_expires_at: u64,    // Boost expiration timestamp
    gifted_today: u32,        // Energy gifted today
    last_gift_reset: u64,     // Last gift reset timestamp
}
```

## Gas Optimization Strategies

1. **Lazy Evaluation**: Energy regeneration calculated only when accessed
2. **Integer Arithmetic**: Avoids floating point operations
3. **Efficient Storage**: Minimal storage writes, updates only when necessary
4. **Batch Operations**: Gift operations update both players efficiently

## Security Considerations

- **Authorization**: All player actions require `require_auth()`
- **Overflow Protection**: Uses saturating arithmetic for time calculations
- **Input Validation**: Validates amounts, timestamps, and addresses
- **Admin Controls**: Contract can be paused by admin in emergencies
- **Token Safety**: Uses Soroban token client for secure transfers

## Testing Coverage

The contract includes comprehensive tests for:
- Energy regeneration over time
- Boost mechanics and expiration
- Energy consumption and limits
- Token-based refills
- Energy gifting between players
- Edge cases and error conditions
- Admin functions and access control

## Integration with Quest System

This contract integrates with the broader quest ecosystem:
- **Puzzle Verification**: Consumes energy before puzzle attempts
- **Reward Token**: Uses reward tokens for instant refills
- **Tournament**: May affect tournament participation
- **Leaderboard**: Energy management affects player activity

## Future Extensions

Potential enhancements for future versions:
- Energy tiers based on player level
- Seasonal events with modified regeneration rates
- Guild-based energy bonuses
- Cross-chain energy synchronization
- Dynamic pricing for token refills based on market conditions