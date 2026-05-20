Status: completed

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Implement three initial condition generators in `galax-init`: Plummer sphere, disk galaxy, and uniform random field. Wire `--init plummer|disk|uniform` into the CLI. Unit tests verify each generator produces correct counts and valid positions.

## Acceptance criteria

- [x] Plummer generator: radial density profile matches analytic ρ(r) ∝ (1 + r²)⁻⁵/²
- [x] Disk generator: surface density approximately exponential, rotation curve approximately flat
- [x] Uniform generator: uniform spatial density and velocity distribution
- [x] `--init plummer --n 10000 --steps 0` generates Plummer distribution and prints stats
- [x] Generated Bodies have valid positions within the simulation bounds
- [x] Unit tests for each generator

## Completion notes

- uniform: random positions in [-50, 50]², random masses [0.1, 10.1]
- plummer: inverse CDF sampling of r², isotropic velocities from escape speed
- disk: exponential radial profile with scale length 5, circular velocities with dispersion
- 3 unit tests: correct count + valid positions for each generator
- CLI --init flag integrated

## Blocked by

- 01-workspace-n2-force.md (needs BodiesSoA)
