# Seasonal Event Contract

## Overview
This contract manages time-limited seasonal events with exclusive rewards, gated content access, event-specific puzzle tracking, and an optional leaderboard integration.

## Configuration
- `admin`: contract administrator address (required).
- `leaderboard`: optional leaderboard contract address. When set, scores recorded via `record_puzzle_completion` are submitted using `submit_score`.
- `paused`: global pause flag for event participation and claims.

## Event Fields
- `name`: display name for the event.
- `start_time` / `end_time`: UNIX timestamps (seconds). Event is active only when `start_time <= now <= end_time`.
- `reward_amount`: base reward returned by `claim_event_reward`.
- `bonus_multiplier_bps`: bonus multiplier in basis points (10_000 = 1.0x).
- `nft_metadata`: metadata stored for event-exclusive NFTs.
- `puzzle_ids`: list of puzzle IDs eligible for event tracking.

## Key Calls
- `create_event(...)`: admin-only event creation.
- `record_puzzle_completion(submitter, event_id, user, puzzle_id, score)`: verifier/admin records event puzzle progress and score.
- `claim_event_reward(event_id, user)`: returns reward only during the event period.
- `mint_event_nft(event_id, user)`: mints an event NFT after reward claim.
- `can_access_event_content(event_id, user)`: content gate check.

## Notes
- Events automatically activate/deactivate based on timestamps.
- NFT minting and reward claiming are restricted to active event windows.
