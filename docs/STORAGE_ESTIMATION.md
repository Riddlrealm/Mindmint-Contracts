# Storage estimation

For each contract, sum entries × bytes per entry × lifetime. Compare against the per-account bucket documented in `docs/PRIVACY_IMPACT_ASSESSMENT.md`.

## Referral contract

`ReferralsList(Address)` stores the complete `Vec<Address>` for one referrer in a single entry. The operational budget for that entry is 5 KiB per referrer. A serialized Soroban address is estimated at 44 bytes; reserving 720 bytes for the storage key, vector framing, and encoding headroom leaves 4,400 bytes for addresses:

```text
(5,120 bytes - 720 bytes) / 44 bytes per address = 100 addresses
```

Therefore `MAX_REFERRALS_PER_USER` is 100. Configuration must not permit a larger value, so the entry remains within the per-account budget as it grows.
