Status: ready-for-agent

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Precompute M2L interaction lists for all nodes using parent-neighbor expansion (children of parent's 3×3 Moore neighborhood, excluding own adjacent cells). Store as `[[u32; 64]; num_nodes]` with separate `[u8; num_nodes]` length array. Implement M2L: convert source node's multipole coefficients to target node's local expansion coefficients using Cartesian derivatives of the softened kernel. Implement L2L: shift a parent's local expansion to each child's center. Implement L2P: evaluate local expansion at each leaf Body's position to produce accelerations. Compare FMM far-field (M2L → L2L → L2P) against direct N² far-field contributions.

## Acceptance criteria

- [ ] Interaction lists computed correctly: every source-target pair is well-separated per the parent-neighbor rule
- [ ] Interaction list length never exceeds 64 entries (assertion at build time)
- [ ] M2L coefficients match direct Taylor expansion of source multipole at target center (test with one source cell)
- [ ] L2L: parent local expansion → child local expansion matches re-expansion at child center
- [ ] L2P: evaluated acceleration matches direct far-field contribution from well-separated cells
- [ ] Rayon `par_iter()` used in M2L and L2P loops

## Blocked by

- 03-p2m-m2m.md (needs multipole expansions)
