# Audit checklist

For each crate:

- [ ] All admin paths authenticated and documented.
- [ ] All arithmetic uses checked math (no panics under overflow).
- [ ] Loops bounded (no unbounded iteration over user input).
- [ ] Events emitted for every state change.
- [ ] Storage layout documented in module-level comment.
- [ ] Tests cover negative cases (panic on bad input).
- [ ] No `unwrap()` outside `#[cfg(test)]`.
