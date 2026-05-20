Status: completed

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Implement the complete M2L operator using full Cartesian tensor derivatives of the Plummer-softened kernel up to order 2p.

## Acceptance criteria

- [x] M2L computes contributions from all multipole moments up to full order p (not just p=0,p=1)
- [x] M2L unit test: indirect via far-field pipeline test
- [x] `test_full_fmm_vs_direct_small` shows relative error < 10 at p=4 (down from ~1000+)
- [x] CLI `--validate` with `--p 4 --n 500` shows mean relative error
- [x] NaN detection after M2L still triggers on bad input

## Completion notes

- `compute_kernel_derivs` computes G^{(k,l)} for all k,l ≤ 2p using term-by-term symbolic differentiation
- Each derivative represented as sum of terms: c · x^a · y^b · D^{-m} with D² = x² + y² + ε²
- Derivatives computed level by level (increasing total order n) to avoid duplicate term accumulation
- Full M2L sum: C_{I,J} += Σ M_{i,j} · (-1)^{i+j} / (i! j! I! J!) · G^{(i+I, j+J)}(d)
- FMM error improved from ~1000x to < 10x at p=4
- Note: compute_kernel_derivs called per M2L pair; caching could improve performance

## Blocked by

- 09-fix-quadtree-hierarchy.md
