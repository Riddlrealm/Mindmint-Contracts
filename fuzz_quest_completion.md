## Description
The quest completion function handles reward calculations and player validation but lacks fuzz testing for edge cases.

## Expected Behavior
Foundry fuzz tests should stress-test the completion logic with randomized inputs.

## Scope

- Reward calculation edge cases
- Re-entrancy scenarios
- Invalid quest IDs and player addresses
- Boundary conditions on timestamps and amounts

## Acceptance Criteria

- Fuzz tests written using Foundry
- No failures across minimum 10,000 runs
- Re-entrancy guard confirmed effective under fuzz