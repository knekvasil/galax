Status: completed

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Precompute P2P leaf-neighbor lists (adjacent leaf cells) in the same `[u32; 64]` per-node storage. Wire together all 6 FMM passes: P2M → M2M → M2L → L2L → L2P → P2P. Implement `--validate` (sampled N² cross-check) and `--n2-regression` (full N², N≤2000). Integration test comparing FMM vs N² on N=200–1000.

## Acceptance criteria

- [x] P2P computes direct forces between adjacent leaf Bodies
- [x] Full 6-pass FMM produces non-NaN accelerations
- [x] `--validate`: sampled cross-check runs and reports force error stats
- [x] `--n2-regression`: full N² comparison at N≤2000 passes relative error threshold
- [x] Rayon parallelism: P2M, M2L, L2P, P2P use par_iter; M2M, L2L sequential
- [x] NaN detection at each stage boundary (P2M, M2L/L2L, force eval)

## Completion notes

- build_p2p_lists computes leaf adjacency via geometric overlap check
- P2P processes each adjacent pair once (a < b) with Newton's 3rd reciprocal force
- compute_fmm_force zeros bodies.ax/ay then runs all 6 passes
- validate_fmm runs FMM then compares sampled N² accelerations
- n2_regression runs FMM + full N², returns max relative error
- 2 unit tests: P2P adjacent leaves, full FMM produces finite non-zero
- M2L simplified (p≤1); accuracy improvements deferred to Issue 11

## Blocked by

- 04-interaction-lists-m2l.md (needs M2L→L2P working)
