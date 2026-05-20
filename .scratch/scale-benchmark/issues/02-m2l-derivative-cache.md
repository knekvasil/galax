Status: completed

## Parent

[scale-benchmark PRD](../PRD.md)

## What to build

Add HashMap cache to M2L `compute_kernel_derivs` lookup, keyed by quantized (dx, dy).

## Acceptance criteria

- [x] M2L with cache enabled produces bit-identical results to uncached version (all 32 tests pass)
- [x] Cache uses Mutex<HashMap<(i64, i64), Vec<f64>>> inside the Rayon-parallel M2L pass
- [x] Read-check/compute/write pattern: lock briefly to check, compute outside lock if miss, lock briefly to store
- [x] All existing tests still pass

## Completion notes

- Cache key: (dx as i64, dy as i64) — quantizing to nearest integer
- Each cell pair at the same quadtree level shares quantized (dx, dy) — high hit rate
- Mutex contention is low: misses only happen on the first encounter of each separation

## Blocked by

None — can start immediately.
