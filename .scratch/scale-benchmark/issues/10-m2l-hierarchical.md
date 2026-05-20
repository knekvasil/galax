Status: ready-for-agent

## Parent

[scale-benchmark PRD](../PRD.md)

## What to build

Restructure the M2L accumulation from flat O(p⁴) contraction into hierarchical radial/angular accumulation. The Cartesian tensor expansion has structure: the contribution of terms with `i+j = n` and `I+J = N` depends primarily on `(n+N)` and the angular separation. By first accumulating over radial shells (sum over all terms with same `i+j`), then applying angular corrections per `(I,J)`, the multiply-add count reduces from O(p⁴) to approximately O(p³).

The key: group terms by `r = i+I` and `s = j+J` (the total derivative order in each axis), then apply the factorial and sign factors hierarchically.

## Acceptance criteria

- [ ] M2L produces bit-identical results to pre-optimization (within machine precision)
- [ ] M2L benchmark at N=10000 drops from ~0.2s to ~0.1s (after symmetry + flatten)
- [ ] All existing tests still pass

## Blocked by

- 09-m2l-flatten.md (needs clean index infrastructure)
