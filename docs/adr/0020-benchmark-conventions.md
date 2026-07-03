# ADR-0020: Benchmark conventions

## Status
Accepted.

## Decision
Benchmarks live under `benches/` per crate. Numbers are pinned in `docs/PERFORMANCE_BENCHMARKS.md` and a regression of more than 20% blocks release.

## Consequences
- Performance drift is caught at PR time.
- Benchmark runs are slow and need caching.
