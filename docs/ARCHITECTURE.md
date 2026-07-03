# Architecture

Mindmint is a workspace of independent Soroban smart-contract crates that compose into a logic-puzzle game on Stellar.

## Layered structure

- **Gameplay** — puzzle_verification, time_attack, multiplayer_match, flash_challenge
- **Social** — guild, referral, reputation, mentorship, social_tipping, social_wager
- **Rewards** — reward_token, reward_vault, achievement_nft, achievement_collection, achievement_sets, completion_certificate, lottery, progressive_jackpot, prize_pool
- **Cross-cutting** — rbac, emergency_pause, proxy, upgrade, multisig_escrow, timelock_vault, governance, governance_token
- **Financial** — escrow, lending, token_swap, yield_farming, royalty_splitter, payment_splitter, charity_donation, community_grant

## Contract communication

Contracts compose through Soroban's cross-contract invocation API. See the `cross_contract` crate for patterns and best practices.

## See also

- `docs/CONTRACT_REFERENCE.md`  — index of every crate
- `docs/SECURITY_MODEL.md`   — authn, authz, pause, reentrancy
- `docs/DEPLOYMENT.md`        — building and deploying contracts
