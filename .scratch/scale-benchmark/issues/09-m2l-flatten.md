Status: ready-for-agent

## Parent

[scale-benchmark PRD](../PRD.md)

## What to build

Flatten the M2L inner loops into a single linear index contraction. The current M2L uses 4-deep nested loops: `for i,j: for outer_i,outer_j`. This recomputes index arithmetic (`moment_index`, `kernel_deriv_idx`) and branching (`sign`, `if g == 0`) on every iteration.

Replace with: precompute flat arrays `src_idx[k]`, `tgt_idx[k]`, `weight[k]`, `deriv_idx[k]` for k = 0..N_terms (2025 at p=4). The M2L inner body becomes a single dot product:

```
local[tgt_idx[K]] += Σ_k M[src_idx[k]] * weight[k] * deriv[deriv_idx[k]]
```

This eliminates all index computation, branching, and factorial divisions from the hot loop.

## Acceptance criteria

- [ ] Precomputed flat index arrays exist (built once per `m2l()` call, or lazily via OnceLock)
- [ ] M2L results are bit-identical to pre-optimization version (all existing tests pass)
- [ ] M2L benchmark at N=10000 drops from ~0.6s to ~0.2s (after symmetry optimization)
- [ ] All existing tests still pass

## Blocked by

- 08-m2l-symmetry.md (flattening is cleaner after symmetric reduction)
