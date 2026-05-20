Status: completed

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Fix `Tree::build` to use proper Morton prefix-based sibling identification instead of grouping arbitrary blocks of 4 consecutive nodes. The correct algorithm: after sorting Bodies by Morton key and creating leaf ranges, group sibling nodes at each level by their common Morton prefix — siblings share a prefix length that determines the parent cell in the quadtree. The parent node's half_width should be exactly `2 × child_half_width` (not derived from children's bounding boxes). Parent centers should be the geometric centers of the sibling group, not mass-weighted child positions.

The tree must correctly identify which nodes are siblings (2–4 per parent, depending on which quadrants are occupied), and must produce consistent cell sizes at each tree level. This fixes M2L partner detection (currently misses partners when only 1 parent exists) and leaf adjacency for P2P.

## Acceptance criteria

- [x] `half_width` of every parent node equals `2.0 × max_child_half_width` (within FP tolerance)
- [x] Sibling groups contain exactly the nodes that share a common quadtree parent cell (2–4 per group)
- [x] `test_full_fmm_vs_direct_small` shows max relative error < 10 (down from ~1000+) due to improved M2L partner coverage
- [x] Existing tree invariants (contiguous ranges, no overlap, parent covers children) still pass
- [x] All existing tests still pass (23 core tests, 3 integrate, 3 init, 2 io)

## Completion notes

- `Tree::build` rewritten to group siblings by `key >> (2 * (level + 1))` Morton prefix (shift increases by 2 per level)
- Parent `half_width` now set to `2.0 × max_child_half_width` instead of computed from bounding box
- Body arrays permuted to match Morton order (latent bug: P2M was computing from unsorted bodies)
- `M2M` iteration changed from reverse to forward order (reverse broke on deeper trees)
- `InteractionLists::build` level computation changed from reverse to forward order
- FMM error improved from ~1000x to < 10x

## Blocked by

None — can start immediately.
