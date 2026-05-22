Status: ready-for-agent

# M2L kernel derivative recurrence (WGSL)

## Parent

`.scratch/gpu-architecture/PRD.md`

## What to build

Implement the closed-form polynomial recurrence for the Plummer-softened kernel derivatives `G^{(k,l)}(dx, dy, ε)` for all `k + l ≤ 2p` as a pure WGSL function in `shaders/shared.wgsl`.

The softened kernel is `G(r) = -1 / sqrt(r² + ε²)` where `r² = dx² + dy²`. Each derivative `G^{(k,l)}` can be expressed as:

```
G^{(k,l)} = P_{k,l}(dx, dy, ε) · (dx² + dy² + ε²)^{-(1+k+l)/2}
```

where `P_{k,l}` is a polynomial computed via a two-term recurrence that depends only on `k, l` and the previous-order polynomials. This recurrence avoids the O(3ⁿ) term explosion of the current CPU implementation (Issue 14), which generates 6561 terms at n=8 for p=4 that partially cancel through FP subtraction.

The recurrence is the same algorithm as Issue 14's suggested approach A (closed-form recurrence via isotropic kernel property), translated to WGSL.

The function is `compute_g_deriv(dx: f32, dy: f32, eps: f32, p: u32) -> array<f32, M>` where `M = (2p+1)(2p+2)/2`. It resides in `shared.wgsl` and is included by the M2L shader.

Validate by implementing the same recurrence in Rust, computing derivatives at known (dx, dy, ε) points, then dispatching a WGSL test shader that computes the same values on GPU and comparing via readback.

## Acceptance criteria

- [ ] WGSL `compute_g_deriv` function in `shared.wgsl` computes all `G^{(k,l)}` for `k+l ≤ 2p`
- [ ] WGSL version matches Rust reference implementation to within `1e-5` relative at 100 random (dx, dy, ε, p) points
- [ ] Recurrence performance: profiled vs the current CPU `compute_kernel_derivs` — the WGSL version should produce correct values without the O(3ⁿ) term explosion (this is a precision fix for Issue 14 on GPU)
- [ ] Test: `test_deriv_recurrence_analytic` — verify known analytic values (G^{(0,0)}, G^{(1,0)}, G^{(0,1)}) match
- [ ] Test: `test_deriv_recurrence_symmetry` — verify `G^{(k,l)} = G^{(l,k)}` (rotational symmetry)
- [ ] The recurrence fits within the WGSL `MAX_COMPUTE_WORKGROUP_SIZE` constraints for the M2L shader register budget

## Blocked by

- `.scratch/gpu-architecture/issues/01-gpu-bootstrap.md` (GPU bootstrapping)
