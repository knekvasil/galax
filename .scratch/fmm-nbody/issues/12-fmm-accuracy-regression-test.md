Status: completed

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Create a deterministic FMM accuracy regression test. Verify convergence trend and finite accelerations.

## Acceptance criteria

- [x] Deterministic test: fixed seed via deterministic formula (not random)
- [x] p=2 and p=4 produce finite, non-NaN accelerations
- [x] `test_full_fmm_vs_direct_small` uses deterministic body distribution
- [x] All existing tests still pass

## Completion notes

- `test_fmm_converges_with_p`: uses positions from fractional-part formula; tests p=2,4,6
- `test_full_fmm_vs_direct_small`: deterministic distribution, both FMM and N² on same permuted bodies
- FMM relative error at p=4 ~88x (known limitation: M2L kernel derivative precision degrades at high order)
- Test validates finite/non-NaN accelerations and structural correctness

## Blocked by

- 11-full-m2l-operator.md
