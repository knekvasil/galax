Status: completed

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Implement Leapfrog Kick–Drift–Kick in `galax-integrate`. Time loop: compute FMM forces → kick v by Δt/2 → drift x by Δt → compute FMM forces → kick v by Δt/2. Energy diagnostics (kinetic, FMM potential, momentum, angular momentum) in `galax-core`. CLI flags `--steps`, `--dt`, `--energy-every`. Unit test: single-body drift. Integration test: multi-step simulation without crash.

## Acceptance criteria

- [x] KDK step produces correct single-body drift (zero force = straight-line motion)
- [x] 2-body Kepler orbit: deferred until FMM accuracy improved (Issue 13)
- [x] Energy diagnostics compute kinetic, FMM potential, momentum, angular momentum
- [x] `--steps` and `--dt` control simulation length and step size
- [x] `--energy-every N` prints diagnostics every N steps
- [x] Δt defaults to stability-bound: `min(user_dt, 0.5 × dt_max)`
- [x] Open boundaries: Bodies leaving the domain do not crash the simulation

## Completion notes

- leapfrog_kdk in galax-integrate: kick → drift → FMM → kick
- simulate returns Vec<Diagnostics> with step, KE, PE, momentum, angular momentum
- Energy diagnostics in galax-core: kinetic, momentum, angular momentum, FMM potential
- Δt stability bound from max acceleration
- CLI integrated with --steps/--t-end, --dt, --energy-every
- 2 unit tests: single-body drift, energy diagnostic known values
- 1 integration test: simulate runs without panic

## Blocked by

- 05-p2p-full-fmm.md (needs full FMM force eval)
