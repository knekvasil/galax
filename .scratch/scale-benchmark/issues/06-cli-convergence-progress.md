Status: completed

## Parent

[scale-benchmark PRD](../PRD.md)

## What to build

Add `--convergence` and `--progress` flags to galax-cli.

## Acceptance criteria

- [x] `--convergence` runs FMM at p=2,4,6,8 and prints error table using validate_fmm
- [x] `--progress` shows progress bar during simulation loop via indicatif
- [x] Both flags work with existing `--init`, `--n`, `--steps` flags

## Completion notes

- convergence uses build_constrained tree for accuracy
- progress bar shows elapsed time, progress, and ETA
- indicatif dependency added to galax-cli

## Blocked by

None — can start immediately.
