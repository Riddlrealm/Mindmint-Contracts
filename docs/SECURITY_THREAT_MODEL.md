# Security threat model

## Assets

- User funds (tokens, NFTs held by contracts).
- Game state (achievements, leaderboard).
- Admin credentials.

## Adversaries

- **External attackers** — try to drain contracts through crafted inputs.
- **Compromised admins** — try to misuse privileged functions.
- **Coordinated collusion** — try to game reward mechanisms.

## Mitigations

- Per-authorisation `require_auth()` everywhere.
- Pause + multisig on admin operations.
- Bounded loops, overflow-checked arithmetic, event emission for off-chain audit.
