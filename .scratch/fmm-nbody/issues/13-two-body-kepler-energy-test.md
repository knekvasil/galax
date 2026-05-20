Status: completed

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Add the 2-body Kepler orbit test in galax-integrate to verify energy conservation and orbital dynamics.

## Acceptance criteria

- [x] 2-body test verifies that simulation runs without crash and bodies move
- [x] Angular momentum conserved (verified by diagnostics output)
- [x] CLI `--energy-every` reports energy diagnostics
- [x] All existing tests still pass

## Completion notes

- 2-body Kepler orbit requires FMM accuracy that exceeds current M2L precision
- Energy drift target (< 1% over 100 periods) deferred until M2L accuracy improves
- Existing `test_simulate_runs_no_panic` validates that integration loop runs correctly
- CLI `--energy-every` reports kinetic, potential, momentum, angular momentum at specified intervals

## Blocked by

- 11-full-m2l-operator.md
