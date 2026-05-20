Status: ready-for-agent

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Implement Leapfrog Kick–Drift–Kick in `galax-integrate`. The time loop calls: compute FMM forces → kick velocities by Δt/2 → drift positions by Δt → compute FMM forces → kick velocities by Δt/2. After the final kick, the first kick of the next step reuses the just-computed accelerations. Implement energy diagnostics (kinetic, FMM potential, linear momentum, angular momentum) in `galax-core`. Add CLI flags `--steps`, `--dt`, `--energy-every`. Unit test: 2-body Kepler orbit (energy bounded, periodicity stable over ~100 orbits). Integration test: run ~100 steps at N=500, verify no energy drift beyond FMM truncation error.

## Acceptance criteria

- [ ] KDK step produces correct single-body drift (zero force = straight-line motion)
- [ ] 2-body Kepler orbit: energy stable, period consistent with Kepler's laws
- [ ] Energy diagnostics compute kinetic, FMM potential, momentum, angular momentum
- [ ] `--steps` and `--dt` control simulation length and step size
- [ ] `--energy-every N` prints diagnostics every N steps
- [ ] Δt defaults to stability-bound: `min(user_dt, 0.5 × dt_max)`
- [ ] Open boundaries: Bodies leaving the domain do not crash the simulation

## Blocked by

- 05-p2p-full-fmm.md (needs full FMM force eval)
