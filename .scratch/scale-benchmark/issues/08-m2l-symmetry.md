Status: ready-for-agent

## Parent

[scale-benchmark PRD](../PRD.md)

## What to build

Exploit the isotropic symmetry of the Plummer-softened kernel in the M2L derivative computation. The kernel is isotropic — `G^{(a,b)}(dx, dy, ε) = G^{(b,a)}(dx, dy, ε)` for the same separation vector. The current `compute_kernel_derivs` stores and computes all (a,b) pairs independently. Replace with a symmetric storage that stores only `a ≥ b` and maps symmetric lookups accordingly. The M2L inner loops are adjusted to iterate over only the unique terms and mirror contributions where symmetry applies.

This is a pure refactor: the mathematical output is identical (within machine precision), just computed with ~half the work.

## Acceptance criteria

- [ ] `compute_kernel_derivs` stores only `a ≥ b` entries (triangular storage, ~45 coeffs instead of 81 at p=4)
- [ ] M2L results are bit-identical to pre-optimization version (all existing tests pass)
- [ ] M2L benchmark at N=10000 drops from ~1.13s to ~0.6s
- [ ] All existing tests still pass

## Blocked by

None — can start immediately.
