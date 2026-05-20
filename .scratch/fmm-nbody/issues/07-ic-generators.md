Status: ready-for-agent

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Implement three initial condition generators in `galax-init`, each producing a `BodiesSoA`: Plummer sphere (spherically symmetric density profile with isotropic velocities), disk galaxy (exponential surface density with circular rotation curve), and uniform random field (random positions and velocities within a square). Wire `--init plummer|disk|uniform` into the CLI. Unit tests verify each generator matches its analytic expectation (density profile for Plummer, rotation curve for disk, uniform density for uniform).

## Acceptance criteria

- [ ] Plummer generator: radial density profile matches analytic ρ(r) ∝ (1 + r²)⁻⁵/²
- [ ] Disk generator: surface density approximately exponential, rotation curve approximately flat
- [ ] Uniform generator: uniform spatial density and velocity distribution
- [ ] `--init plummer --n 10000 --steps 0` generates Plummer distribution and prints stats
- [ ] Generated Bodies have valid positions within the simulation bounds
- [ ] Unit tests for each generator

## Blocked by

- 01-workspace-n2-force.md (needs BodiesSoA)
