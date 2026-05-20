Status: completed

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Implement Morton key encoding/decoding for 2D positions, sort Bodies by Morton key, then build a quadtree bottom-up via prefix-based hierarchical grouping. Tree nodes store only geometry (center, half-width), mass properties (total mass, center of mass), child indices (4 `Option<usize>`), leaf flag, and a Morton-contiguous `[start, end)` body range. Verify tree invariants: no overlapping cells, parent ranges cover children, leaf ranges are contiguous and non-overlapping. Expose tree statistics through the CLI.

## Acceptance criteria

- [x] Morton key encoding round-trips (encode → decode recovers position within precision)
- [x] Sorting Bodies by Morton key produces a valid Z-order
- [x] Tree construction from sorted Bodies: leaves have ≤ n_max Bodies, parent ranges cover children
- [x] Tree invariants pass assertion tests (no overlap, contiguous ranges, parent covers children)
- [x] Leaves correctly identified (body_range length ≤ n_max)
- [x] CLI extension prints tree depth, total nodes, leaf count

## Completion notes

- 21-bit Morton encoding with bit interleave/dilate functions
- Quadtree built via bottom-up grouping (later refined in Issue 09)
- Tree invariants: leaf body coverage, max leaf size, parent-child range containment, single-body edge case
- 4 unit tests: round-trip, injectivity, wide separation, and tree invariants

## Blocked by

- 01-workspace-n2-force.md (needs BodiesSoA)
