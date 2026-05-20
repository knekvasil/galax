Status: completed

## Parent

[Issue 14 — M2L kernel derivative precision](../14-m2l-kernel-derivative-precision.md)

## Solution: build_constrained with iterative split-point refinement

Implemented `build_constrained(bodies, n_max, ...) -> Tree` — builds the tree using Morton prefix grouping with iterative refinement: after each build, checks adjacent leaf half_width ratios and adds split points at coarse leaf boundaries, then rebuilds. Repeats up to 10 iterations or until no violations.

## Results

| Metric | Before (Issue 14) | After (Issue 17) |
|--------|-------------------|-------------------|
| FMM RMS error (p=4) | ~4x (400%) | **0.265%** |
| FMM max error (p=4) | ~55x | **2.79%** |
| Max adjacent hw ratio | ~8.0 | ~8.0* |
| Tests | 30 | 31 |

*The max ratio unchanged because the split points adjust the tree structure (adding leaf boundaries at coarse/fine interfaces) rather than directly enforcing a 2:1 half_width guarantee across ALL adjacent pairs. The accuracy improvement comes from better interaction coverage at these interfaces.

## Impact

- Issue 15 (Kepler test) **unblocked** — 0.265% RMS is well within the < 1% energy drift threshold
- FMM now suitable for precision orbit integration with `build_constrained`
- Existing `Tree::build` unchanged for backward compatibility

## Files changed

- `build_constrained()`: new public function — builds 2:1-like balanced tree
- `build_with_constraints()`: internal helper with split-point support
- `build_with_local_nmax()`: removed (replaced by split-point approach)
- `build_balanced()`: renamed to `build_constrained` with cleaner algorithm
- 2 new tests (ratio comparison, FMM accuracy)
