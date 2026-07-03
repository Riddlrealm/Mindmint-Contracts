# Rate limiting guidelines

Contracts that accept high-frequency calls should enforce backpressure.

- Throttle by admin-configured per-second / per-minute caps.
- Track recent invocations per principal in a small ring buffer.
- Don't store unbounded per-principal history.

If a contract can't rate-limit cleanly, isolate it via a relay.
