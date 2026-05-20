Status: completed

## Parent

[scale-benchmark PRD](../PRD.md)

## What to build

Parallelize `compute_n2_force` outer loop with Rayon.

## Acceptance criteria

- [x] Rayon `into_par_iter()` on outer loop — each body computed independently
- [x] Results collected as Vec<(f64, f64)> then written back sequentially
- [x] All 32 tests pass (bit-identical to sequential version)
- [x] Performance: 4-8x speedup on 8-core machine at N=2000

## Completion notes

- Uses collect-then-write pattern to avoid mutable borrow conflicts with Rayon
- Reads bodies.x[i], bodies.y[i] once per iteration (hoisted out of inner loop)

## Blocked by

None — can start immediately.
