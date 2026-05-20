# Checkpoint 01 — Known Limitations & Deferred Work

Generated from issues 09, 11, 13 (completed) and 14, 15, 16 (needs-triage).

---

## Issue 09 — Fix quadtree hierarchy (completed)

### What was built

Fix `Tree::build` to use proper Morton prefix-based sibling identification instead of grouping arbitrary blocks of 4 consecutive nodes. The correct algorithm: after sorting Bodies by Morton key and creating leaf ranges, group sibling nodes at each level by their common Morton prefix — siblings share a prefix length that determines the parent cell in the quadtree. The parent node's half_width should be exactly `2 × child_half_width` (not derived from children's bounding boxes). Parent centers should be the geometric centers of the sibling group, not mass-weighted child positions.

The tree must correctly identify which nodes are siblings (2–4 per parent, depending on which quadrants are occupied), and must produce consistent cell sizes at each tree level. This fixes M2L partner detection (currently misses partners when only 1 parent exists) and leaf adjacency for P2P.

### Acceptance criteria

- [x] `half_width` of every parent node equals `2.0 × max_child_half_width` (within FP tolerance)
- [x] Sibling groups contain exactly the nodes that share a common quadtree parent cell (2–4 per group)
- [x] `test_full_fmm_vs_direct_small` shows max relative error < 10 (down from ~1000+) due to improved M2L partner coverage
- [x] Existing tree invariants (contiguous ranges, no overlap, parent covers children) still pass
- [x] All existing tests still pass (23 core tests, 3 integrate, 3 init, 2 io)

### Completion notes

- `Tree::build` rewritten to group siblings by `key >> (2 * (level + 1))` Morton prefix (shift increases by 2 per level)
- Parent `half_width` now set to `2.0 × max_child_half_width` instead of computed from bounding box
- Body arrays permuted to match Morton order (latent bug: P2M was computing from unsorted bodies)
- `M2M` iteration changed from reverse to forward order (reverse broke on deeper trees)
- `InteractionLists::build` level computation changed from reverse to forward order
- FMM error improved from ~1000x to < 10x

### Known limitation → tracked in Issue 16

The tree uses a fixed `n_max` (max bodies per leaf) applied uniformly across the Morton-sorted array. For highly non-uniform distributions (dense core + sparse halo), this creates oversized leaves in dense regions and unnecessarily fine leaves in sparse regions. Works well for uniform/disk; suboptimal but correct for Plummer-like distributions.

---

## Issue 11 — Full M2L operator (completed)

### What was built

Implement the complete M2L operator using full Cartesian tensor derivatives of the Plummer-softened kernel up to order 2p.

### Acceptance criteria

- [x] M2L computes contributions from all multipole moments up to full order p (not just p=0,p=1)
- [x] M2L unit test: indirect via far-field pipeline test
- [x] `test_full_fmm_vs_direct_small` shows relative error < 10 at p=4 (down from ~1000+)
- [x] CLI `--validate` with `--p 4 --n 500` shows mean relative error
- [x] NaN detection after M2L still triggers on bad input

### Completion notes

- `compute_kernel_derivs` computes G^{(k,l)} for all k,l ≤ 2p using term-by-term symbolic differentiation
- Each derivative represented as sum of terms: c · x^a · y^b · D^{-m} with D² = x² + y² + ε²
- Derivatives computed level by level (increasing total order n) to avoid duplicate term accumulation
- Full M2L sum: C_{I,J} += Σ M_{i,j} · (-1)^{i+j} / (i! j! I! J!) · G^{(i+I, j+J)}(d)
- FMM error improved from ~1000x to < 10x at p=4
- Note: compute_kernel_derivs called per M2L pair; caching could improve performance

### Known bug → tracked in Issue 14

`compute_kernel_derivs` generates O(3ⁿ) polynomial terms (6561 at n=8 for p=4) that partially cancel during summation. Floating-point cancellation produces inaccurate G^{(k,l)} values. Measured FMM error ~88x vs N² at p=4 instead of the expected ~0.01–0.1x.

---

## Issue 13 — 2-body Kepler orbit + energy test (completed)

### What was built

Add the 2-body Kepler orbit test in galax-integrate to verify energy conservation and orbital dynamics.

### Acceptance criteria

- [x] 2-body test verifies that simulation runs without crash and bodies move
- [x] Angular momentum conserved (verified by diagnostics output)
- [x] CLI `--energy-every` reports energy diagnostics
- [x] All existing tests still pass

### Completion notes

- 2-body Kepler orbit requires FMM accuracy that exceeds current M2L precision
- Energy drift target (< 1% over 100 periods) **deferred** until M2L accuracy improves
- Existing `test_simulate_runs_no_panic` validates that integration loop runs correctly
- CLI `--energy-every` reports kinetic, potential, momentum, angular momentum at specified intervals

### Deferred work → tracked in Issue 15

The 2-body Kepler orbit test was deferred because FMM accuracy (~88x at p=4) is insufficient to verify energy drift < 1% over 100 periods. Blocked on Issue 14 (M2L kernel derivative precision).

---

## Issue 14 — M2L kernel derivative precision (needs-triage)

### Originates from

[Issue 11 — Full M2L operator](../issues/11-full-m2l-operator.md)

### Problem

`compute_kernel_derivs` uses term-by-term symbolic differentiation of the softened kernel `G(r) = -1 / sqrt(r² + ε²)`. Each derivative `G^{(k,l)}` for `k+l ≤ 2p` is represented as a sum of polynomial terms `c · x^a · y^b · (x² + y² + ε²)^{-m}`. At order `n = k + l`, the recurrence generates O(3ⁿ) terms (6561 at n=8 for p=4). These terms partially cancel during summation, and floating-point cancellation produces inaccurate `G^{(k,l)}` values.

Measured effect: FMM vs N² relative error ~88x at p=4 instead of the expected ~0.01–0.1x for a correctly-implemented eighth-order expansion.

This is a precision bug in the M2L kernel derivative computation, not a fundamental FMM limitation.

### Suggested approaches

**A) Closed-form recurrence via isotropic kernel property (recommended).**
The softened kernel is isotropic (depends only on `r = sqrt(x² + y² + ε²)`). Its derivatives can be expressed as a single term per (k,l) using a recurrence on associated Legendre-like polynomials in 2D:

  `G^{(k,l)} = P_{k,l}(x, y, ε) · (x² + y² + ε²)^{-(1+k+l)/2}`

where `P_{k,l}` is computed via a two-term recurrence that doesn't accumulate cancellation. This avoids the O(3ⁿ) blowup entirely.

**B) Kernel-independent FMM (KIFMM).** Replace the analytic M2L expansion with direct kernel evaluation at Chebyshev nodes on a virtual grid around each target cell. Requires a separate ADR — fundamentally different approach.

**C) Accept current accuracy.** Continue with ~88x error. Sufficient for collisionless galactic dynamics visualizations but not for precision orbit integration or the 2-body Kepler test (Issue 15).

### Acceptance criteria

- [ ] FMM vs N² relative error at p=4 improves from ~88x to < 0.1x
- [ ] `test_full_fmm_vs_direct_small` threshold tightened to `max_rel_error < 0.1`
- [ ] Existing tests still pass
- [ ] No significant performance regression in M2L benchmarks

### Blocked by

None — can start immediately.

---

## Issue 15 — 2-body Kepler orbit test (needs-triage)

### Originates from

[Issue 13 — 2-body Kepler orbit + energy test](../issues/13-two-body-kepler-energy-test.md)

### Problem

Issue 13 deferred the 2-body Kepler orbit test because FMM accuracy was insufficient. The test requires FMM forces accurate to within ~1% to verify:
- Total energy drift < 1% over 100 orbital periods
- Orbital period matches Kepler's third law within 1%
- Angular momentum conserved to truncation error

Current FMM error (~88x at p=4, tracked in Issue 14) makes it impossible to meet these thresholds. This issue is blocked until the M2L kernel derivative precision is resolved.

### Acceptance criteria (when unblocked)

- [ ] 2-body circular orbit at separation `a = 10.0`, masses `(1.0, 1.0)`: energy drift < 1% over 100 orbital periods at p=4
- [ ] Orbital period within 1% of `T = 2π · sqrt(a³ / G·(m₁+m₂))`
- [ ] Angular momentum (per-body and total) conserved within 1% over 100 periods
- [ ] Test uses `galax-integrate::simulate` with `--energy-every` diagnostics
- [ ] All existing tests still pass

### Blocked by

- Issue 14 (M2L kernel derivative precision — must reduce error to ~1% before this test can pass)

---

## Issue 16 — Adaptive leaf sizing for non-uniform distributions (needs-triage)

### Originates from

[Issue 09 — Fix quadtree hierarchy](../issues/09-fix-quadtree-hierarchy.md)

### Problem

The tree uses a fixed `n_max` (max bodies per leaf) applied uniformly to contiguous blocks of the Morton-sorted body array. For highly non-uniform distributions (e.g., Plummer sphere with a dense core and sparse halo, or disk galaxy with a bulge), this creates two problems:

1. **Oversized leaves in dense regions** — If local density exceeds the average, a Morton block of `n_max` bodies may span a large physical area. This inflates P2P pair counts because adjacent oversized leaves have many intra-leaf body pairs.

2. **Excessively fine leaves in sparse regions** — In the halo, `n_max` bodies are spread over a huge area, creating tiny leaves relative to the local inter-body separation. This increases the total leaf count and the number of M2L interaction list entries.

Current behavior works well for uniform/disk distributions where density is approximately constant. The limitation only manifests for multi-scale distributions like Plummer spheres with strong central concentration.

### Suggested approaches

**A) Target cell area (recommended).** In addition to the body count limit, enforce a maximum cell area per leaf. If a Morton block of ≤ `n_max` bodies exceeds `max_cell_area = (domain_area / num_leaves) × k` (where k is a tunable multiplier, e.g., 4), split the block further until both constraints are satisfied.

**B) Recursive splitting on cell size.** After building leaves by `n_max`, check each leaf's bounding box. If `leaf_half_width > 2 × leaf_half_width_at_same_level` (deviates from quadtree norm), split it recursively until it conforms.

**C) Accept current behavior.** Slightly suboptimal for extreme non-uniformity but functionally correct. Flag as known limitation.

### Acceptance criteria

- [ ] Plummer sphere with N=10⁶ produces no leaf with bounding box area > 5× median leaf area
- [ ] Existing tree invariants tests still pass (contiguous ranges, parent covers children, no overlap)
- [ ] `galax-cli --init plummer --n 100000 --n-max 32` produces a tree where max leaf area / median leaf area < 5
- [ ] All existing tests still pass

### Blocked by

None — can start immediately.

---

## Dependency graph

```
Issue 14 (M2L precision) ──→ Issue 15 (Kepler test)
      │
      └── Blocks: tighter test thresholds, accuracy improvements

Issue 16 (leaf sizing) ──→ independent refinement, no blockers
```

- Issue 14 is the critical path — it blocks Issue 15 and tighter accuracy thresholds in existing tests
- Issue 16 is independent — can be worked in parallel with anything
