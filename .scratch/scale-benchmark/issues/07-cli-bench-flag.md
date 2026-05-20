Status: completed

## Parent

[scale-benchmark PRD](../PRD.md)

## What to build

Add `--bench` flag to galax-cli for per-pass timing.

## Acceptance criteria

- [x] `galax-cli --bench --n 500` prints per-pass timing table
- [x] Timing includes: tree_build, P2M, M2M, M2L, L2L, L2P, P2P, full_fmm, n2
- [x] Runs at N=100,500,1000,5000,N by default

## Completion notes

- Bench logic inline in galax-cli (no dependency on galax-bench)
- Timing uses std::time::Instant
- M2L confirmed as dominant cost at all N

## Blocked by

None — standalone (no dependency on galax-bench crate).
