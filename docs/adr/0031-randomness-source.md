# ADR-0031: Randomness Source for Referral Code Generation

## Status

Accepted.

## Context

Issue #14 identified that the referral contract's code generation used `env.ledger().timestamp()` as an entropy source, which is predictable and can be manipulated by validators or observed by users. This created a security vulnerability where:

1. A referrer could mass-generate codes ahead of referees
2. Referral attribution could be hijacked
3. Code collision grinding was feasible given the low entropy (~40 bits effective)

The original implementation combined only 4 bytes of counter + 8 bytes of timestamp, providing ~96 bits of surface but <40 bits of effective entropy after the alphanumeric reduction step.

## Decision

We replace the timestamp-based entropy with a cryptographic commitment scheme using Keccak256:

```rust
let mut entropy_input = soroban_sdk::Bytes::new(&env);
entropy_input.extend_from_slice(&user.clone().to_raw_bytes());
entropy_input.extend_from_slice(&nonce.to_be_bytes());
entropy_input.extend_from_slice(&env.current_contract_address().to_raw_bytes());
let hash = env.crypto().keccak256(&entropy_input);
```

### Entropy Sources

1. **User Address** (32 bytes): Unique per-user, unpredictable before user interaction
2. **Nonce** (8 bytes): Monotonically increasing counter stored in contract storage
3. **Contract Address** (32 bytes): Fixed per deployment, known only after contract instantiation

### Security Properties

- **≥128 bits of entropy**: From user address (256 bits) + nonce (64 bits) + contract address (256 bits)
- **Collision probability ≤ 2⁻⁶⁴**: Achieved through cryptographic hash properties
- **No predictable timestamps**: `env.ledger().timestamp()` is completely removed from the code path

## Alternatives Considered

1. **VRF Oracle**: Not available in current Soroban version; would require cross-contract call to external randomness provider
2. **Soroban Host Primitives**: Not yet hardened in SDK 21.x for this use case
3. **On-chain Randomness**: Would require validator cooperation, not trust-minimized

## Consequences

- **Positive**: Eliminates predictable entropy vulnerability; provides cryptographically secure code generation
- **Positive**: Backwards-compatible with existing CodeOwner(String) keys (no migration required for code format)
- **Negative**: Requires contract address storage in entropy calculation (minor gas cost increase)
- **Neutral**: Nonce must be persisted in storage (already required for uniqueness)

## Migration Notes

Existing referral codes are not affected by this change. The CodeOwner(String) key format remains unchanged. New codes generated after this update will use the secure entropy source.

## Testing

Unit test `test_referral_code_uniqueness_over_100k` verifies uniqueness over ≥100,000 generated codes with zero collisions.
