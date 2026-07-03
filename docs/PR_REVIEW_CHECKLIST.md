# PR review checklist

Use this for every meaningful PR.

## Correctness

- [ ] Logic matches the issue.
- [ ] Negative cases tested (panic with correct Error).
- [ ] No `unwrap` outside tests.

## Security

- [ ] `require_auth()` on every state-changing path.
- [ ] No unbounded iteration over caller input.
- [ ] Arithmetic is checked where it could overflow.

## Operability

- [ ] Events emitted for every state change.
- [ ] Storage layout documented if changed.
- [ ] Backwards-compatible (or migration path written).
