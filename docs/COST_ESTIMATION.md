# Cost estimation

How to estimate Soroban resource costs.

## Components

- **CPU instructions** — per-instruction fee × op count × invocations.
- **Memory bytes** — per byte read/write × count.
- **Storage entries** — per entry × size × lifetime.
- **Events** — per emitted event × size × emitted count.

## Estimating

1. Profile under realistic load.
2. Multiply per-invocation cost by monthly invocation estimate.
3. Add storage lifetime cost.

Always re-estimate after a contract change.
