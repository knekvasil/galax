Status: completed

## Parent

[scale-benchmark PRD](../PRD.md)

## What to build

Flatten M2L 4-deep nested loops into a single linear index contraction with precomputed plan.

## Acceptance criteria

- [x] Precomputed `M2lTerm` plan with src_idx, tgt_idx, weight, deriv_idx cached per p via OnceLock
- [x] M2L inner body reduced to single loop: `local[tgt] += M[src] * weight * deriv[deriv_idx]`
- [x] All 32 core tests pass (bit-identical results)

## Completion notes

- Plan built once per p (0..=16) and stored in `m2l_plan()` via OnceLock
- Each term pre-computes: moment_index, factorial, sign, kernel_deriv_idx
- Hot loop eliminates all index arithmetic, branching, and factorial divisions
- MAX_P increased to 17 to accommodate pp=16 in plan builder

## Blocked by

- 08-m2l-symmetry.md (was expected to reduce terms first; closed as N/A)
