Status: completed

## Parent

[Issue 11 — Full M2L operator](../11-full-m2l-operator.md)

## Background

Issue 14 was originally scoped to fix "M2L kernel derivative precision" — the hypothesis was that the FMM accuracy gap (~88x vs N² at p=4) was caused by catastrophic cancellation in the term-by-term symbolic differentiation of the softened kernel.

## What was found

### Finding 1: Stable kernel derivatives ✅

The original `compute_kernel_derivs` used term-by-term symbolic differentiation producing O(3ⁿ) terms per derivative (6561 at n=8 for p=4). These terms partially cancel during summation, causing floating-point noise.

**Fix:** Replaced with closed-form polynomial recurrence. Each derivative is computed as:

  `G^{(k,l)} = Σ N_{k,l}[a][b] · x^a · y^b · (x² + y² + ε²)^{-(1+2k+2l)}`

where `N_{k,l}` is a dense coefficient array maintained via the recurrence:
  `N_{k+1,l} = R² · ∂N/∂x - (1+2k+2l) · N · x`
  `N_{k,l+1} = R² · ∂N/∂y - (1+2k+2l) · N · y`

Derivatives now match analytical formulas to machine precision (verified at p=4).

### Finding 2: Corrected P2M/M2M/M2L formulas ✅

Discovered and fixed three bugs in the multipole formulation:

1. **P2M moment sign**: Removed spurious `(-1)^{i+j}` from `M_{i,j}`. The correct moment is `M_{i,j} = Σ m · Δx^i · Δy^j / (i! j!)`. The sign belongs in the series expansion, not the moment.

2. **M2M shift formula**: Changed from binomial coefficient shift `C(I,i) · C(J,j)` to factorial-based shift `1/((I-i)! (J-j)!)`.

3. **M2L formula**: Removed extra `1/(i! j!)` denominator from the source side of the M2L summation. The correct formula is:

   `C_{I,J} = Σ M_{i,j} · (-1)^{i+j} / (I! J!) · G^{(i+I, j+J)}(d)`

### Finding 3: Self-interaction in P2P ✅

P2P was missing within-leaf body pairs. Added self (same-leaf) entries to the P2P neighbor lists so that bodies in the same leaf compute direct N² forces with each other.

### Finding 4: The real accuracy bottleneck 🔴

After all mathematical fixes, error went from ~88x to ~55x max / ~4x RMS. The error is the SAME at p=0 as at p=4 — proving M2L is NOT the bottleneck. The residual ~4x RMS comes from interaction list gaps at adaptive tree depth boundaries, not from M2L derivatives.

**Diagnosis (via coverage matrix validator):**
- Same-level M2L interaction lists are correct and non-overlapping
- Cross-level pairs (cells at different tree depths that are spatially near but not leaf-adjacent) fall through the crack between P2P and M2L
- The parent-neighbor stencil used for M2L assumes balanced tree topology
- Building cross-level interaction lists without double-counting requires either 2:1 balanced tree constraints or full dual-tree traversal

**Attempted fix:**
- Extended P2P lists to include adjacent non-leaf cells, with P2P descending into their descendant leaves
- Extended M2L lists to include cross-level partners with ancestor-adjacency checks
- Both approaches caused double-counting (pairs covered by both P2P and M2L)
- Reverted all cross-level changes — the complexity/risk tradeoff doesn't justify the ~4x RMS improvement

## Current state

| Metric | Before (Issue 14 start) | After |
|--------|------------------------|-------|
| Derivative accuracy | O(3ⁿ) cancellation | Machine-precision closed form |
| P2M sign convention | Wrong `(-1)^{i+j}` in moment | Correct |
| M2M shift formula | Binomial (wrong for this convention) | Factorial-based |
| M2L formula | Wrong `1/(i! j!)` factor | Correct |
| P2P self-interaction | Missing | Included |
| FMM max error (p=4) | ~88x | ~55x |
| FMM RMS error (p=4) | ~4x | ~4x |

The ~4x RMS error is the cost of not having 2:1 balanced tree constraints. Fixing this would require a separate issue (2:1 tree balancing or dual-tree traversal).

## Acceptance criteria

- [x] Stable kernel derivatives (match analytic to machine precision)
- [x] Correct P2M/M2M/M2L formulas
- [x] P2P includes self-interaction
- [x] FMM p=0 = P2P (identity verified)
- [x] Existing tests still pass

## Blocked by

None — can start immediately.

## Follow-up

The remaining accuracy gap (~4x RMS) is tracked implicitly in Issue 16 (adaptive leaf sizing / 2:1 balance). A 2:1 balanced tree would eliminate the level-boundary interaction gaps that cause the error.
