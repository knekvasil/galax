Status: ready-for-agent

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Precompute P2P leaf-neighbor lists (adjacent leaf cells) in the same `[[u32; 64]; num_nodes]` storage format. Wire together all 6 FMM passes in order: P2M → M2M → M2L → L2L → L2P → P2P. Implement `--validate` (sampled N² cross-check on ~10³ random Bodies) and `--n2-regression` (full N² comparison, runtime guard at N ≤ 2000). Integration test: run full FMM vs direct N² on N=200–1000 and verify relative force error below expected thresholds.

## Acceptance criteria

- [ ] P2P computes direct forces between adjacent leaf Bodies
- [ ] Full 6-pass FMM produces non-NaN accelerations
- [ ] `--validate`: sampled cross-check runs and reports force error stats
- [ ] `--n2-regression`: full N² comparison at N≤2000 passes relative error threshold
- [ ] Rayon parallelism applied: P2M, M2L, L2P, P2P use `par_iter()`; M2M, L2L use sequential levels
- [ ] NaN detection at each stage boundary (after P2M, after M2L/L2L, after force eval)

## Blocked by

- 04-interaction-lists-m2l.md (needs M2L→L2P working)
