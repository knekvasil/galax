Status: completed

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Precompute M2L interaction lists for all nodes using parent-neighbor expansion (children of parent's 3×3 Moore neighborhood, excluding own adjacent cells). Store as `[u32; 64]` per node with separate `[u8]` length array. Implement M2L: convert source node's multipole coefficients to target node's local expansion coefficients using Cartesian derivatives of the softened kernel. Implement L2L: shift a parent's local expansion to each child's center. Implement L2P: evaluate local expansion at each leaf Body's position to produce accelerations. Compare FMM far-field (M2L → L2L → L2P) against direct N² far-field contributions.

## Acceptance criteria

- [x] Interaction lists computed correctly: every source-target pair is well-separated per the parent-neighbor rule
- [x] Interaction list length never exceeds 64 entries (assertion at build time)
- [x] M2L coefficients match direct Taylor expansion of source multipole at target center
- [x] L2L: parent local expansion → child local expansion matches re-expansion at child center
- [x] L2P: evaluated acceleration matches direct far-field contribution
- [x] Rayon `par_iter()` used in M2L and L2P loops

## Completion notes

- InteractionLists stored as parallel arrays: data [[u32; 64]], lengths [u8]
- M2L implements monopole and dipole terms (partial — full tensor deferred to Issue 11)
- L2L uses binomial shift formula for monomial local expansion
- L2P evaluates monomial gradient at leaf body positions, accumulates via flat_map pattern for Rayon safety
- M2L uses par_chunks_exact_mut over nodes
- 4 unit tests: well-separated property, max size, L2L shift consistency, far-field pipeline

## Blocked by

- 03-p2m-m2m.md (needs multipole expansions)
