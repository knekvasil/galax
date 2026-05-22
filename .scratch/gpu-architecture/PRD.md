Status: ready-for-agent

# PRD: GPU-Accelerated FMM N-body Simulation

## Problem Statement

The current CPU-based FMM N-body simulation uses Rayon parallelism and achieves usable performance up to ~3,000 Bodies on an Apple M1 Air (8GB unified memory). Beyond 10,000 Bodies, per-step time becomes prohibitive for interactive use. The target hardware has an 8-core GPU sharing the same unified memory pool as the CPU, but the GPU is entirely unused. There is no GPU compute path.

A GPU-accelerated path is needed to push the practical simulation scale to 100,000+ Bodies while maintaining interactive per-step latencies (<1s per step at 10k Bodies, <5s per step at 100k Bodies).

## Solution

A new crate `galax-gpu` implementing all six FMM passes (P2M, M2M, M2L, L2L, L2P, P2P) as wgpu compute shaders in WGSL, sharing a wgpu `Device`/`Queue` owned by the application layer. The tree build, Leapfrog KDK integration, and diagnostics remain on CPU. GPU-visible buffers are mapped for zero-copy access on Apple Silicon unified memory, with strict stage-based ownership boundaries — no shared mutable memory between CPU and GPU.

Validation uses a three-layer strategy: CPU-vs-GPU cross-comparison during development (detects implementation bugs), occasional GPU N² debug kernel (isolates GPU numerical behaviour), and tolerance-based FMM-vs-N² validation in production (verifies physical correctness).

## User Stories

1. As a user, I want the simulation to use the GPU for force computation, so that I can run simulations with 10x more Bodies than the CPU path allows on my M1 hardware.

2. As a user, I want the `--gpu` flag on `galax-cli` to activate the GPU compute path, so that I can choose between CPU and GPU backends without changing my workflow.

3. As a user, I want the GPU FMM to produce forces within 1% RMS relative error vs N² at p=4, so that the simulation is physically meaningful.

4. As a user, I want the GPU path to use the same tree and interaction lists as the CPU path, so that the spatial decomposition and interaction topology is identical between backends.

5. As a user, I want position/velocity/mass data to reside in mapped GPU buffers that the CPU can write and the GPU can read without copies, so that per-step overhead is minimized on unified memory.

6. As a user, I want NaN detection at GPU stage boundaries via an atomic error flag, so that silent corruption from NaN propagation is caught before it corrupts integration.

7. As a user, I want energy diagnostics (kinetic, potential, momentum, angular momentum) to be computed on CPU from force readback, so that I can use the same diagnostic tooling as the CPU path.

8. As a user, I want `galax-viz` to accept a `--gpu` flag and share the wgpu device between compute and rendering, so that visualization of 100k+ Bodies is practical.

9. As a user, I want the CPU and GPU paths to produce identical FMM forces at f64 precision when compared per-pass during development, so that I can verify the GPU WGSL translation is correct.

10. As a user, I want a development-mode `--gpu-crosscheck` flag that runs both CPU and GPU FMM and reports per-body force differences, so that I can debug shader bugs.

11. As a user, I want the WGSL shaders to be compiled via `build.rs` at Rust build time, so that shader syntax errors are caught at `cargo build` time rather than at runtime.

12. As a user, I want the interaction lists to be stored on GPU in compact offset-based format (contiguous indices array + per-node offset array), so that GPU memory bandwidth is not wasted on fixed-size padding.

13. As a user, I want the M2L kernel derivative recurrence to be computed inline in WGSL (not precomputed or table-lookup), so that the GPU FMM avoids CPU↔GPU synchronization for derivative tables.

14. As a user, I want the simulator to use f32 end-to-end on GPU (positions, masses, velocities, accelerations, expansions), so that arithmetic throughput is maximized on M1 GPU hardware where f64 is 1/16th rate.

15. As a user, I want the tree to remain static during a simulation run (no per-step rebuild), so that interaction lists and GPU buffers remain valid across all steps.

16. As a user, I want preallocated max-size GPU buffers set at init time, so that there is zero allocation overhead during simulation steps.

17. As a user, I want the Leapfrog KDK integration to remain on CPU, so that integration logic stays in familiar Rust and the GPU path remains a pure force accelerator.

18. As a user, I want per-particle leaf ownership stored in a GPU buffer (`leaf_id: array<u32>`), so that P2P and L2P shaders can locate their owning leaf in O(1) without binary search or tree traversal on GPU.

19. As a user, I want a deterministic GPU command submission order (P2M → M2M → M2L → L2L → L2P → P2P) with explicit buffer barriers, so that data hazards between passes are impossible.

## Implementation Decisions

### New crate: `galax-gpu`

A new workspace member with no public API dependency on wgpu types in its public interface. The crate exposes:

- `GpuContext` — holds wgpu `Device`, `Queue`, all buffers, six compute pipelines, and the uniform parameter buffer.
- `GpuContext::new(device: Arc<Device>, queue: Arc<Queue>, params: GpuConfig) -> Result<Self, GpuError>` — initializes all buffers and compiles WGSL pipelines. `GpuConfig` contains `max_n`, `max_nodes`, `p`, `eps`, `max_interactions`. Returns `Result` for recoverable init failures.
- `GpuContext::step(&self, bodies: &BodiesSoA, tree: &Tree, m2l_lists: &InteractionLists, p2p_lists: &InteractionLists)` — the main entry point. Encodes all 6 WGSL dispatches, submits to queue, panics on device loss or NaN flag.
- `GpuContext::read_accelerations(&self, ax: &mut [f64], ay: &mut [f64])` — readback GPU dynamics buffer into CPU f64 arrays for diagnostics.

### Buffer system

Six GPU buffer groups, all preallocated to `Config::max_*` sizes:

1. **Geometry buffer** — `array<vec2<f32>>` of length `max_n`. Positions only. Mapped write from CPU.
2. **Mass buffer** — `array<f32>` of length `max_n`. Read-only from GPU. Uploaded at tree build.
3. **Dynamics buffer** — two `array<vec2<f32>>` for velocities and accelerations. Mapped read/write from CPU (acceleration readback for diagnostics).
4. **Expansion buffer** — flat `array<f32>` for multipole and local coefficients, stride `(p+1)(p+2)/2`. Two regions: multipole `[0..max_nodes*stride)` and locals `[max_nodes*stride..2*max_nodes*stride)`.
5. **Interaction list buffer** — `indices: array<u32>` (all partner node IDs concatenated) + `offsets: array<u32>` of length `max_nodes+1` (prefix-sum index into indices). Built by compacting CPU `InteractionLists` at tree build time.
6. **Per-body leaf_id buffer** — `array<u32>` of length `max_n`. Maps body index to its leaf node ID. Computed on CPU after tree build.
7. **Tree node buffers** — pass-specific minimal subsets of `Node` per buffer: `centers: array<vec2<f32>>` (M2L), `leaf_ranges: array<vec2<u32>>` (P2P/L2P: body_start/body_end packed as u32 pairs), `node_parent: array<i32>` (M2M/L2L). One child indices buffer `[i32; 4]` per node for M2M/L2L.

### Uniform parameter buffer

A single `uniform` buffer (or push constants where backend supports) updated once per step:

```
struct SimParams {
    n: u32,
    num_nodes: u32,
    num_leaves: u32,
    p: u32,
    eps: f32,
    _pad: f32,
}
```

### WGSL organization

- `galax-gpu/shaders/shared.wgsl` — common WGSL functions: `moment_index(i, j, p)`, `num_moments(p)`, `expand_idx(i, stride)`, kernel derivative recurrence `compute_g_deriv(dx, dy, eps, p) -> array<f32, MAX_MOMENTS>`, binomial/factorial constants via `override` or const-eval, interaction list accessors.
- `galax-gpu/shaders/p2m.wgsl`, `m2m.wgsl`, `m2l.wgsl`, `l2l.wgsl`, `l2p.wgsl`, `p2p.wgsl` — per-pass entry points. Each includes shared.wgsl via build.rs concatenation.
- `galax-gpu/build.rs` — reads `shaders/shared.wgsl` and each pass `.wgsl`, concatenates into standalone modules, writes to `$OUT_DIR`, optionally runs `naga` for validation.
- `galax-gpu/shaders/n2_debug.wgsl` — O(N²) direct force kernel for diagnostic use only.

### Per-pass dispatch strategy

| Pass | Dispatch dimensions | Thread work | Buffer reads | Buffer writes |
|------|-------------------|-------------|--------------|---------------|
| P2M | `num_leaves` × 1 × 1 | One leaf per thread, loops over `[body_start, body_end)`, accumulates multipole moments | geometry, mass, leaf_ranges | expansions (multipole region) |
| M2M | 1 dispatch per tree level, `num_nodes_at_level` × 1 × 1 | One node per thread, reads child expansions, shifts+sums into parent | expansions (multipole, children), child_indices | expansions (multipole, parent) |
| M2L | `num_nodes` × 1 × 1 | One node per thread, loops over partner indices from interaction list, reads partner multipole, computes kernel derivatives inline, accumulates local expansion | centers (self + partners), expansions (multipole, partners), list_data, list_offsets | expansions (local) |
| L2L | 1 dispatch per level (reverse), `num_nodes_at_level` × 1 × 1 | One node per thread, reads parent local, shifts down to child | expansions (local, parent), child_indices | expansions (local, child) |
| L2P | `n` × 1 × 1 | One body per thread, reads leaf local expansion via leaf_id, evaluates derivatives at body offset from leaf center, writes acceleration | leaf_ranges, leaf_id, expansions (local), geometry | dynamics (accelerations) |
| P2P | `n` × 1 × 1 | One body per thread, reads leaf P2P partner list, loops over partner body range, computes direct softened 1/r force | leaf_id, leaf_ranges, list_data, list_offsets, geometry, mass | dynamics (accelerations, additive) |

### M2L kernel derivative recurrence (WGSL)

The closed-form recurrence for the isotropic Plummer-softened kernel `G(r) = -1 / sqrt(r² + ε²)` computes `G^{(k,l)}(dx, dy, ε)` for all `k + l ≤ 2p` via a polynomial recurrence that avoids the O(3ⁿ) term blowup of the current CPU implementation. The recurrence produces `P_{k,l}(dx, dy, ε) · (dx² + dy² + ε²)^{-(1+k+l)/2}` where `P` is computed via two-term recurrence. This is the same algorithm described in issue 14's suggested approach A.

### Validation three-layer system

**Layer A (development):** `GpuContext::crosscheck(&self, cpu_fmm_result: &BodiesSoA) -> Vec<(usize, f64)>` — dispatches the GPU pipeline, reads back accelerations, compares per-body against CPU `compute_fmm_force` output. Reports max relative error per body. Gated behind `--gpu-crosscheck` flag. Panics if max error exceeds a tunable threshold (default 1e-4 relative for development).

**Layer B (diagnostic):** An optional N² WGSL compute shader. `GpuContext::n2_debug(&self) -> Vec<(f32, f32)>` — dispatches `n2_debug.wgsl` which computes direct O(N²) forces on GPU. Used only during manual debugging sessions to isolate GPU numerical behaviour from CPU comparison.

**Layer C (production):** `validate_fmm_gpu()` analogue — builds tree, runs GPU FMM, runs GPU N² on a sampled subset, reports RMS and max relative error. No CPU involvement. The error tolerance is looser than CPU (target < 1% RMS at p=4) because f32 precision is dominated by FMM truncation error.

### Modified existing crates

**`galax-core`:**
- No changes to existing public API.
- May expose internal `Node` field accessors and `InteractionLists` raw data for buffer upload if visibility requires.

**`galax-integrate`:**
- New function `leapfrog_kdk_gpu(bodies, tree, m2l_lists, p2p_lists, gpu_ctx, p, softening, dt)` that calls `gpu_ctx.step(...)` instead of `compute_fmm_force(...)`. The KDK arithmetic remains CPU.
- `simulate_gpu()` variant that uses `leapfrog_kdk_gpu`.

**`galax-cli`:**
- New `--gpu` flag (bool).
- When `--gpu` is set, creates wgpu device/queue, initializes `GpuContext`, and calls `simulate_gpu`.
- When `--gpu-crosscheck` is set (implies `--gpu`), runs both CPU and GPU FMM, prints error report.

**`galax-viz`:**
- wgpu device created in main, shared between viz renderer and `GpuContext`.
- New `--gpu` flag.
- Simulation viewer creates `GpuContext` from the existing wgpu device.

## Testing Decisions

A good test verifies external behaviour and invariants, not implementation details. Tests never mix correctness assertions with performance measurement.

### galax-gpu unit tests

- **Buffer round-trip:** Write bodies to mapped GPU geometry/dynamics buffer, dispatch a trivial pass-through shader, read back, verify equality. Validates the mapped buffer and f32↔f64 conversion boundary.
- **Interaction list compaction:** Build CPU InteractionLists, run compaction, verify GPU offset array matches sequential CPU iteration of the same lists.
- **Kernel derivative recurrence (CPU WGSL emulation):** Implement the WGSL recurrence in Rust, compare against analytic derivatives for several (dx, dy, ε, p) values. Verifies the recurrence math before GPU translation.
- **Expand index mapping:** Verify moment_index(i, j, p) and stride calculations match CPU `moment_index`.

### galax-gpu integration tests

- **GPU FMM vs CPU N²** on small N (≤500) at p=4 with constrained tree. Max relative error < 10% (f32 tolerance). Verifies the full GPU pipeline produces physically reasonable forces.
- **GPU FMM vs CPU FMM crosscheck** at N=200 at p=4. Forces match to within 1e-3 relative (f32 implementation vs f64 reference). Validates WGSL translation correctness.
- **NaN detection:** Insert a NaN position into the geometry buffer. Verify the atomic error flag is set after the GPU dispatch.
- **Determinism:** Run GPU FMM twice on the same input. Verify bit-identical accelerations.

### Prior art

Existing 33 core tests in `galax-core` follow the same pattern: public interface tests, no mocks, no implementation detail coupling. The kernel derivative recurrence test follows the same analytic-verification approach as the existing `test_kernel_derivs_analytic` test. Cross-comparison tests follow the pattern of `test_full_fmm_vs_direct_small`.

## Out of Scope

- Adaptive tree rebuild during simulation (static tree per run)
- Mixed precision (f32 end-to-end on GPU)
- Multi-GPU or distributed GPU
- Hardware beyond Apple Silicon (wgpu provides backend portability but this PRD targets M1)
- Replacing the CPU FMM path (both paths coexist)
- Real-time GPU-to-video rendering pipeline (galax-viz rendering is separate)
- Kernel-independent FMM or black-box FMM
- 3D simulation
- Adaptive time stepping

## Further Notes

- ADR-0003 (precomputed fixed-size interaction lists) explicitly anticipated GPU portability. The compact offset-based format is the natural GPU evolution of that design.
- The inline WGSL kernel derivative recurrence directly addresses Issue 14 (M2L kernel derivative precision) — the closed-form recurrence avoids the O(3ⁿ) term explosion of the current CPU implementation, potentially fixing the ~88x error on GPU.
- Three new ADRs are recommended to record the architectural decisions behind: (1) f32 precision end-to-end on GPU, (2) wgpu backend choice on Apple Silicon, and (3) body buffer layout by access domain.
- The GPU path naturally scales to 100k+ Bodies on M1 hardware. Beyond ~500k Bodies, GPU memory bandwidth for interaction list traversal becomes the primary bottleneck, at which point further optimization (workgroup-level sharing, subgroup ops, or periodic tree rebuild) would be warranted.
