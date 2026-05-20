Status: completed

## Parent

[scale-benchmark PRD](../PRD.md)

## What to build

HashMap-based spatial index for O(1) neighbor queries, keyed by (level, grid_i, grid_j).

## Acceptance criteria

- [x] `SpatialIndex::build(tree) -> Self` produces correct mapping
- [x] `neighbors_of(node_id) -> [Option<usize>; 8]` finds neighbors in O(1)
- [x] All index-found neighbors are verified adjacent via `cells_adjacent`
- [x] 32 core tests pass (1 new test added)

## Completion notes

- Uses HashMap<(u32, i64, i64), usize> mapping (level, grid_i, grid_j) -> node_id
- Level computed from tree structure (leaf=0, parent=1, root=max)
- Grid coordinates: cell_size = 2*half_width, i = round((cx - x_min) / cell_size)
- neighbor_of tries same level first, then finer (descendant), then coarser (ancestor)
- Works best with build_constrained trees (consistent cell sizes at each level)

## Blocked by

None — can start immediately.
