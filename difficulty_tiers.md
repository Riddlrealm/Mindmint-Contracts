## Description
All quests currently have the same reward structure regardless of complexity or challenge level.

## Expected Behavior
Quests should have difficulty tiers that influence reward multipliers.

## Proposed Changes

```
enum Difficulty { Easy, Medium, Hard, Legendary }
```

- Add `Difficulty difficulty` to the quest struct
- Apply reward multipliers: Easy 1x, Medium 1.5x, Hard 2x, Legendary 3x

## Acceptance Criteria

- Difficulty is set at quest creation
- Reward payout respects the difficulty multiplier
- Tests cover all four difficulty levels