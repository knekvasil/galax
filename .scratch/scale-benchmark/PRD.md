Status: ready-for-agent

# PRD: Scale Testing & Performance Optimization

## Problem Statement

The FMM N-body simulation has been verified for correctness at small N (≤500 bodies) with 0.265% RMS accuracy, verified integrator energy conservation, and a full CLI. However, it has never been tested at the target scale of 10⁶+ Bodies. The primary scaling bottlenecks are unknown, and there is no CI pipeline to prevent regressions. Without this work, the simulation cannot be used for its intended purpose of studying galactic-scale dynamics.

## Solution

A systematic scale-testing and performance-optimization phase comprising:

1. **Benchmark hierarchy** — per-pass benchmarks (P2M, M2M, M2L, L2L, L2P, P2P) at multiple N scales (10³, 10⁴, 10⁵, 10⁶) to identify where time is spent
2. **M2L kernel derivative caching** — the current M2L recomputes all kernel derivatives per interaction pair. Cache shared derivatives across pairs with the same (dx, dy) separation vector
3. **N² force Rayon parallelism** — `compute_n2_force` is currently sequential; parallelize the outer loop
4. **P2P adjacency optimization** — the O(n) `neighbors_geometric` scan per node becomes prohibitive at 10⁶ bodies; replace with spatial index
5. **GitHub Actions CI** — automate `cargo test`, `cargo bench`, and a regression smoke test at N=1000
6. **galax-viz** — real-time visualization crate for debugging and demos

## User Stories

1. As a user, I want the simulation to complete an FMM step at 10⁶ Bodies in under 10 seconds on a laptop, so that I can run interactive experiments.

2. As a user, I want to run `cargo bench` and see per-pass timing breakdowns, so that I can identify performance regressions.

3. As a user, I want the N² validation force to use all CPU cores via Rayon, so that `--n2-regression` completes in reasonable time at N≤2000.

4. As a user, I want CI to automatically run tests and benchmarks on every push, so that I catch regressions early.

5. As a user, I want to visualize the simulation in real time, so that I can see the Bodies moving and debug unexpected behavior.

6. As a user, I want the M2L pass to cache kernel derivative computations, so that the per-step time decreases at high expansion order.

7. As a user, I want the tree construction to complete at 10⁶ Bodies in under 1 second, so that I don't spend more time building the tree than running the simulation.

8. As a user, I want to see a progress indicator during long simulations, so that I know the simulation hasn't hung.

9. As a user, I want to run a convergence demo (`--convergence`) that shows FMM error vs p across p=2,4,6,8, so that I can choose the right accuracy-performance tradeoff.

## Implementation Decisions

### Module sketch

**galax-core (modified):**

- `compute_n2_force` — parallelized outer loop via `par_iter()` for O(N²) → O(N² / cores) speedup
- `neighbors_geometric` — replaced by spatial HashMap-based lookup for O(1) neighbor queries during tree construction and P2P list building. Key: `(level, grid_i, grid_j) -> node_id`
- M2L kernel derivative caching — precompute derivative tables for quantized (dx, dy) separation vectors. The current implementation rebuilds the polynomial term list for each pair; cache by `(dx_quantized, dy_quantized, eps, p)` to reduce repeated work
- `build_constrained` — already optimized; benchmark at 10⁶ to verify target

**galax-bench (new crate):**

Standalone benchmarking binary for large-N timing. Separate from criterion to avoid overhead.

```rust
// Proposed interface
pub struct BenchReport {
    pub n: usize,
    pub passes: Vec<PassTiming>,
}

pub struct PassTiming {
    pub name: &'static str,    // "tree_build", "p2m", "m2m", etc.
    pub wall_s: f64,
}
```

Runs at N = 10³, 10⁴, 10⁵, 10⁶ and prints timing table. Uses `std::time::Instant`, not criterion.

**galax-viz (new crate):**

Real-time visualization using `minifb` (lightweight, cross-platform). Renders Bodies as colored dots on a 2D canvas.

- Library: `galax-viz` crate with `SimulationViewer` struct
- Two modes: real-time (connects to simulation via shared state) and replay (reads snapshot files)
- No physics logic — pure rendering

**galax-cli (modified):**

- `--convergence` flag: runs FMM at p=2,4,6,8 and prints error-vs-p table
- `--progress` flag: shows progress bar via `indicatif`
- `--bench` flag: runs timing benchmarks at specified N, prints table

**CI (new):**

- `.github/workflows/ci.yml` with:
  - `cargo test --workspace` (31 tests, must pass)
  - `cargo bench -- --sample-size 10` (smoke, not statistically valid but catches major regressions)
  - Regression smoke test: `galax-cli --init uniform --n 1000 --steps 5 --validate`

### Performance targets

| Operation | Target at N=10⁶ | Priority |
|-----------|-----------------|----------|
| Tree construction | < 1s | P1 |
| Full FMM step (p=4) | < 10s | P1 |
| M2L pass (dominant cost) | < 5s | P1 |
| P2P pass | < 3s | P1 |
| P2M + M2M + L2L + L2P sum | < 2s | P2 |

### M2L kernel derivative caching design

Current: `m2l()` calls `compute_kernel_derivs(dx, dy, eps, p)` per interaction pair. This builds polynomial coefficient lists and evaluates them — O(p⁴) per call. With 42k nodes × ~27 M2L partners × p=4, this is ~45M derivative evaluations per step.

Fix: precompute a table of `G^{(k,l)}(dx, dy, ε)` over a grid of quantized separation vectors at tree-building time. During M2L, look up the nearest entry and interpolate if needed. The table size is `(2 * MAX_LEVEL + 1)²` entries × `(2p+1)²` derivatives ≈ negligible memory.

Simpler approach for first pass: hash-map cache keyed by quantized `(dx as i64, dy as i64)`. Since many M2L pairs share separation vectors (cells at the same level have quantized center positions), the hit rate will be high.

### Spatial index for neighbor queries

Replace the current O(n) `neighbors_geometric` scan with a `HashMap<(u32, i64, i64), usize>`. Built during tree construction. Neighbor query becomes O(1) → O(8) with fallback to coarser/finer levels.

Used by:
- `build_p2p_lists` — finding adjacent leaf neighbors
- `build_constrained` — checking 2:1 violations

### galax-viz rendering approach

`minifb` window (800×600). Each frame:
1. Clear buffer
2. Map each Body's (x, y) to pixel coordinates
3. Draw a small circle for each Body
4. Color by velocity magnitude or mass

Frame rate: target 30fps. At 10⁶ Bodies, rendering is the bottleneck (not physics). Use point sprites or simple pixel plotting. Option to decimate (render 1 in every 1000 Bodies for preview).

## Testing Decisions

- **Per-pass correctness** — no new unit tests for optimizations (existing 31 tests verify correctness). New tests only for:
  - Spatial index: verify neighbor lookup works for all node types (leaf, internal, root)
  - M2L cache: verify cached results match uncached results to machine precision
- **Benchmark tests** — criterion for small-N (N=200), `galax-bench` for large-N. CI runs criterion smoke only (too slow to run full large-N bench on every push)
- **CI regression gate** — `galax-cli --init uniform --n 1000 --steps 5 --validate` outputs `mean_rel_err < 0.1`. If this fails, the PR is rejected
- **galax-viz** — visual inspection only; no automated tests
- **Prior art** — existing 31 tests in the workspace follow the same pattern: public interface tests, no mocks, no implementation detail coupling

## Out of Scope

- GPU acceleration (CPU-only via Rayon)
- Adaptive time stepping
- 3D simulation
- Python bindings
- WebAssembly target
- Kernel-independent FMM
- Production-grade visualization (galax-viz is a debug/demo tool)
- Distributed/multi-node parallelism

## Further Notes

- Existing benchmarks at N=200 show ~0.8ms per FMM step at p=4. Extrapolating at O(N) gives ~4s at N=10⁶. The target of <10s leaves margin for non-linear overheads.
- `build_constrained` adds ~0.1s at N=500. At N=10⁶, expected ~1-2s. If this exceeds the 1s target, build with `Tree::build` instead (faster, slightly less balanced).
- The existing criterion benchmarks need to be updated to include `build_constrained` as a separate case.
