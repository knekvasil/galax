Status: completed

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Replace dynamic `Vec` allocations in hot numerical loop bodies with fixed-size stack arrays. Precompute factorial and binomial tables once at module level using OnceLock. Add criterion benchmarks for FMM passes.

## Acceptance criteria

- [x] All Vec allocations removed from P2P, L2P, M2L inner loops (use `[f64; MAX_P]` on stack)
- [x] Factorial/binomial tables computed once, stored as OnceLock
- [x] `cargo bench` runs and reports per-pass timing
- [x] All existing tests still pass (23 core tests)

## Completion notes

- Added `const MAX_P: usize = 16` and `fill_powers` helper function
- `cached_binom` and `cached_fact` use `OnceLock<Vec<Vec<f64>>>` / `OnceLock<Vec<f64>>`
- P2M, M2M, L2L, L2P all use `[f64; MAX_P]` stack arrays instead of `vec![1.0; p+1]`
- criterion bench file: `benches/fmm.rs` with full_fmm and n2_force benchmarks

## Blocked by

None — can start immediately.
