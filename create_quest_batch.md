## Description
Creating multiple quests requires separate transactions, which is expensive and slow during large game deployments.

## Expected Behavior
Admins or quest creators should be able to create multiple quests in a single transaction.

## Proposed Changes

```
function createQuestBatch(QuestParams[] calldata quests) external;
```

## Acceptance Criteria

- Batch creation works for arrays of any valid size
- Gas usage is measurably lower than equivalent individual calls
- Tests cover batch success and partial failure scenarios