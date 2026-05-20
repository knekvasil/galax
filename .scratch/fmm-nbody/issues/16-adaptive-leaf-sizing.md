Status: closed

## Parent

[Issue 09 — Fix quadtree hierarchy](../09-fix-quadtree-hierarchy.md)

## Problem

The tree uses a fixed `n_max` (max bodies per leaf) applied uniformly to the Morton-sorted body array. For highly non-uniform distributions (Plummer sphere with dense core + sparse halo), this creates oversized leaves in the core and fine leaves in the halo, impacting P2P and M2L performance at large N.

## Why this is closed

**Superseded by Issue 17 (build_constrained).** The constraint-driven split-point approach from Issue 17 addresses the practical effect of this issue: when a leaf at a coarse/fine interface would create accuracy problems, the coarse leaf is split. FMM accuracy is now 0.265% RMS at p=4 — well within requirements.

The remaining concern (performance at 10⁶ scale) is a future optimization, not a correctness issue. If performance profiling at scale reveals a bottleneck from leaf size variation, this can be reopened with specific benchmarks.

## Suggested approaches (if revisited)

**A) Target cell area.** Enforce max cell area per leaf in addition to body count limit.

**B) build_constrained refinement.** The existing split-point approach could be extended to also split leaves whose cell area exceeds a configurable threshold.

## Acceptance criteria (unmet — superseded)

- [ ] Plummer sphere with N=10⁶ produces no leaf with bounding box area > 5× median leaf area
- [ ] Existing tree invariants tests still pass (contiguous ranges, parent covers children, no overlap)
- [ ] `galax-cli --init plummer --n 100000 --n-max 32` produces a tree where max leaf area / median leaf area < 5
- [ ] All existing tests still pass
