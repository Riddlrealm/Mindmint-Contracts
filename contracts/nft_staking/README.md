# NFT Staking & Passive Rewards (`nft-staking`)

Stake achievement NFTs to accrue passive token rewards **per ledger**.

## How it works

- **Locking:** `stake` transfers the NFT into the staking contract address (the NFT is not transferable/burnable by the original owner while staked).
- **Accrual:** rewards accrue as:
  - `(current_ledger - last_claim_ledger) * tokens_per_ledger`
- **Claiming:** `claim_rewards` mints rewards to the staker without requiring an unstake.
- **Unstaking:** `unstake` requires a **48h minimum** stake duration and auto-claims pending rewards.

## Admin setup

This contract expects two layers of configuration:

1. **Reward rate per rarity tier**
   - `set_rarity_config(admin, rarity, tokens_per_ledger)`
2. **Rarity tier per NFT token id**
   - `set_token_rarity(admin, token_id, rarity)`

> Note: rewards are paid via the `reward_token` contract using `mint(minter=staking_contract, ...)`. The `reward_token` admin must authorize the staking contract as a minter (call `authorize_minter(staking_contract_id)` on the reward token contract).

## Public functions

- `stake(staker, token_id)`
- `claim_rewards(staker, token_id)`
- `unstake(staker, token_id)` (48h minimum)
- `pending_rewards(token_id)` (view)
- `get_position(token_id)` (view)

## Events

- `NFTStaked(token_id, staker)`
- `NFTUnstaked(token_id, staker)`
- `RewardsClaimed(token_id, (staker, amount))`

