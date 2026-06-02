## Description
There is currently no way to halt the contract in case of an exploit or vulnerability discovery.

## Expected Behavior
The contract owner should be able to pause all quest creation and completions instantly.

## Proposed Changes

- Integrate OpenZeppelin `Pausable`
- Add `whenNotPaused` modifier to `createQuest()` and `completeQuest()`

## Acceptance Criteria

- Owner can pause and unpause the contract
- All state-changing functions respect the pause state
- Tests cover paused and unpaused scenarios