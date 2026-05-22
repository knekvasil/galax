Status: ready-for-agent

# Full FMM pipeline (P2M, M2M, M2L, L2L, L2P)

## Parent

`.scratch/gpu-architecture/PRD.md`

## What to build

Implement the four remaining FMM passes as WGSL compute shaders and wire all 6 passes (P2P from Slice 2, this slice adds P2M, M2M, M2L, L2L, L2P) together in `GpuContext::step()` to produce a complete GPU FMM force computation.

**P2M (Particle-to-Multipole):** One thread per leaf node. Reads body positions and masses from the geometry/mass buffers for `[body_start, body_end)`, accumulates multipole expansion coefficients into the expansion buffer. Uses the same Cartesian moment formulas as the CPU `p2m()`.

**M2M (Multipole-to-Multipole):** One dispatch per tree level (leaf → root). One thread per node at the current level. Reads child multipole expansions from the expansion buffer, applies the Cartesian multipole translation operator (shift), and sums into the parent's multipole slot.

**M2L (Multipole-to-Local) — the hot pass:** One thread per node. Uses the compact M2L interaction lists (index/offset buffers). For each partner, reads the partner's multipole expansion, computes the displacement vector `(center_x - partner_x, center_y - partner_y)`, evaluates the inline kernel derivative recurrence (from Slice 3), and accumulates the local expansion. Sequential over partners within each thread — no atomics.

**L2L (Local-to-Local):** One dispatch per tree level (root → leaf). One thread per node. Reads parent local expansion, applies the Cartesian local translation operator (shift), and writes into the child's local slot.

**L2P (Local-to-Particle):** One thread per body. Reads the body's leaf local expansion (via `leaf_id`), evaluates the expansion at the body's position relative to the leaf center, and writes the resulting acceleration to the dynamics buffer. Additive with P2P results.

**Orchestration:** `GpuContext::step()` encodes all dispatches in order: `p2m → m2m_loop → m2l → l2l_loop → l2p → p2p`, with explicit buffer barriers between passes to prevent data hazards. The uniform `SimParams` buffer is updated once and used by all dispatches.

Tree node GPU buffers are initialized from the CPU `Tree` at init time: centers (`array<vec2<f32>>` for M2L), leaf_ranges (`array<vec2<u32>>` for P2P/L2P), child_indices (`array<vec4<i32>>` for M2M/L2L), parent_ids (`array<i32>` for M2L). These are preallocated to max capacity and populated at tree build time.

Validate by running the full GPU FMM on N=500 at p=4 with constrained tree, reading back accelerations, and comparing against the CPU `compute_fmm_force()`. Max relative error should be < 1% at p=4 (f32 vs f64 + different accumulation order).

## Acceptance criteria

- [ ] P2M WGSL shader produces multipole expansions matching CPU `p2m()` within f32 tolerance
- [ ] M2M WGSL shader (multi-dispatch level-by-level) matches CPU `m2m()`
- [ ] M2L WGSL shader with inline kernel derivatives matches CPU `m2l()` within f32 tolerance
- [ ] L2L WGSL shader (multi-dispatch level-by-level, reverse order) matches CPU `l2l()`
- [ ] L2P WGSL shader matches CPU `l2p()`
- [ ] Full GPU FMM pipeline (P2M→M2M→M2L→L2L→L2P→P2P) produces accelerations matching CPU `compute_fmm_force()` with max relative error < 1% at p=4 for N=500
- [ ] Test: `test_gpu_full_fmm_vs_cpu_full_fmm` in galax-gpu integration tests
- [ ] `galax-cli --gpu --init plummer --n 10000 --steps 10` runs the full GPU pipeline end-to-end
- [ ] NaN propagation is caught by the atomic error flag: if any pass produces NaN (e.g., zero softening + zero separation), the error flag is set and step panics

## Blocked by

- `.scratch/gpu-architecture/issues/02-p2p-near-field.md` (P2P kernel)
- `.scratch/gpu-architecture/issues/03-kernel-derivative-recurrence.md` (kernel derivative WGSL)
