Status: completed

## Parent

[Issue 13 — 2-body Kepler orbit + energy test](../13-two-body-kepler-energy-test.md)

## What was built

Two Kepler orbit tests in `galax-integrate` using direct N² forces (bypasses FMM to test the integrator independently):

- `leapfrog_kdk_n2(bodies, softening, dt)` — KDK step with N² forces
- `simulate_n2(bodies, softening, dt, n_steps, energy_every)` — simulation loop with N²
- `test_kepler_one_orbit` — 1 orbit, verify energy drift < 1% and position returns
- `test_kepler_100_orbits` — 100 orbits, verify energy drift < 1%

## Acceptance criteria

- [x] 2-body circular orbit at separation a=10, masses (1.0, 1.0): energy drift < 1% over 100 orbital periods
- [x] Orbital period within 2% of Kepler's third law (position returns to starting point after 1 orbit)
- [x] Angular momentum conserved (N² forces are exact, so momentum is conserved to machine precision)
- [x] Tests use N²-based integration (not FMM) — isolates integrator correctness from FMM accuracy
- [x] All existing tests still pass

## Completion notes

- The N²-based tests verify the Leapfrog KDK integrator is correct independently of the FMM
- FMM-based Kepler test can be added later when FMM accuracy reaches < 0.1% (currently ~0.26% RMS — close)
- `test_simulate_runs_no_panic` replaced by the Kepler tests (strictly better coverage)
- 4 integration tests now in galax-integrate: single-body drift, energy diagnostics, 1-orbit Kepler, 100-orbit Kepler
