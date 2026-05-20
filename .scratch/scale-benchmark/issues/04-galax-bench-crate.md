Status: completed

## Parent

[scale-benchmark PRD](../PRD.md)

## What to build

Standalone benchmark binary that times each FMM pass at multiple N scales.

## Acceptance criteria

- [x] `galax-bench` crate exists, builds with `cargo build`
- [x] Runs at N=100,500,1000,5000,10000 by default (configurable via CLI arg)
- [x] Each pass timed individually, results in markdown-style table
- [x] Confirmed: M2L is dominant cost (82% at N=10000)

## Results (N=10000)

| Pass | Time (s) | % of total |
|------|----------|-----------|
| tree_build | 0.008 | 0.6% |
| interaction_lists | 0.114 | 8.3% |
| P2M | 0.005 | 0.4% |
| M2M | 0.012 | 0.9% |
| M2L | 1.126 | 82.0% |
| L2L | 0.014 | 1.0% |
| L2P | 0.006 | 0.4% |
| P2P | 0.131 | 9.5% |
| full_fmm | 1.373 | 100% |

Extrapolating to N=10⁶: M2L ~112s (needs optimization to hit 10s target).

## Blocked by

None — can start immediately.
