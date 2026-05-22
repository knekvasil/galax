Status: closed

## Parent

[scale-benchmark PRD](../PRD.md)

## What was attempted

Store only `k ≥ l` entries in the kernel derivative buffer by exploiting symmetry `G^{(a,b)} = G^{(b,a)}`.

## Why closed

The symmetry is `G^{(a,b)}(dx, dy, ε) = G^{(b,a)}(dy, dx, ε)` — it swaps the spatial arguments, not just the derivative indices. In M2L, (dx, dy) are the fixed separation between source and target cell centers. Swapping derivative indices without swapping (dx, dy) gives the wrong value unless dx = dy. The optimization doesn't apply in this form.

All changes reverted. All 32 tests pass.

## Blocked by

None — can start immediately.
