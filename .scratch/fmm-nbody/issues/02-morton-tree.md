Status: ready-for-agent

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Implement Morton key encoding/decoding for 2D positions, sort Bodies by Morton key, then build a quadtree bottom-up via prefix-based hierarchical grouping. Tree nodes store only geometry (center, half-width), mass properties (total mass, center of mass), child indices (4 `Option<usize>`), leaf flag, and a Morton-contiguous `[start, end)` body range. Verify tree invariants: no overlapping cells, parent ranges cover children, leaf ranges are contiguous and non-overlapping.

Expose tree statistics (depth, node count, leaf count) through the CLI.

## Acceptance criteria

- [ ] Morton key encoding round-trips (encode → decode recovers position within precision)
- [ ] Sorting Bodies by Morton key produces a valid Z-order
- [ ] Tree construction from sorted Bodies: leaves have ≤ n_max Bodies, parent ranges cover children
- [ ] Tree invariants pass assertion tests (no overlap, contiguous ranges, parent covers children)
- [ ] Leaves correctly identified (body_range length ≤ n_max)
- [ ] CLI extension prints tree depth, total nodes, leaf count

## Blocked by

- 01-workspace-n2-force.md (needs BodiesSoA)
