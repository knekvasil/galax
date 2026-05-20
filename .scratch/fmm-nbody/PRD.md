Status: ready-for-agent

# PRD: 2D CPU-based FMM Gravitational N-body Simulation

## Problem Statement

There is no working implementation of a 2D gravitational N-body simulation using the Fast Multipole Method in this repo. Existing N-body codes in the ecosystem are either brute-force O(N²) (does not scale to 10⁶+ Bodies), GPU-only (not CPU-portable), or use the incorrect harmonic kernel formulation (complex Laurent FMM for a `1/r` gravitational kernel). A correct, scalable, CPU-parallel FMM implementation is needed for studying galactic dynamics at 10⁶–10⁷ Body scale.

## Solution

A Rust workspace implementing a 2D gravitational N-body simulation using the Fast Multipole Method with Cartesian Taylor expansions, Plummer softening, quadtree decomposition via Morton sort, and Rayon-based parallelism. The simulation produces per-step snapshots (binary dump), energy/momentum diagnostics, and supports three-tier validation (sampled N² cross-check, small-N full N² regression, invariant monitoring).

## User Stories

1. As a user, I want to run a gravitational simulation with 10⁶+ Bodies, so that I can study galactic-scale dynamics.

2. As a user, I want the simulation to use the Fast Multipole Method, so that total cost scales as O(N) per step instead of O(N²).

3. As a user, I want the FMM to use correct Cartesian Taylor expansions, so that the gravitational `1/r` (softened) kernel is accurately approximated.

4. As a user, I want Plummer softening applied to all Gravitational Interactions, so that forces remain finite when Bodies pass close together.

5. As a user, I want to choose initial conditions from Plummer sphere, disk galaxy, and uniform field generators, so that I can study different dynamical scenarios.

6. As a user, I want to set the number of Bodies and integration steps from the command line, so that I can control simulation scope.

7. As a user, I want the simulation to use Leapfrog (KDK) integration, so that energy is conserved to within the FMM truncation error over long runs.

8. As a user, I want to configure the expansion order p, MAC parameter θ, and softening length ε, so that I can trade accuracy for performance.

9. As a user, I want the time step Δt to be automatically bounded by a physics stability criterion, so that I don't accidentally diverge.

10. As a user, I want to override the time step with a user-provided cap, so that I can control temporal resolution.

11. As a user, I want binary snapshot output written periodically, so that I can resume or visualize the simulation later.

12. As a user, I want energy/momentum diagnostics logged every N steps, so that I can monitor drift and stability.

13. As a user, I want to validate the FMM accuracy against direct N² computation on a randomly sampled subset, so that I trust the approximation error.

14. As a user, I want to run a full N² regression check on small N (≤2000 Bodies), so that I can verify correctness in CI.

15. As a user, I want the simulation to use all available CPU cores via Rayon, so that performance scales with hardware.

16. As a user, I want the tree to be built bottom-up from Morton-sorted Bodies, so that the data layout is cache-friendly and interaction lists are cheap to construct.

17. As a user, I want the simulation to handle open boundaries, so that Bodies can freely leave the simulation domain.

18. As a user, I want NaN detection at FMM stage boundaries (after P2M, after M2L/L2L, after force evaluation), so that silent corruption is caught early.

19. As a user, I want to load a pre-existing snapshot as initial conditions, so that I can continue a previous simulation.

## Implementation Decisions

### Core data layout

All Body state is stored as SoA (Structure of Arrays): 7 flat `Vec<f64>` for x, y, vx, vy, mass, ax, ay. The `BodiesSoA` struct owns these arrays and provides index-based access. No per-Body struct exists.

### Tree

A quadtree built bottom-up via Morton sort and prefix-based hierarchical grouping. Bodies are sorted by Morton key; leaves are contiguous `[start, end)` ranges in the sorted array with at most `n_max` Bodies each. Parents merge adjacent sibling ranges upward. The `Node` struct stores only geometry (center, half-width), total mass, center of mass, child indices, leaf flag, and body range. Multipole and local expansion coefficients are stored externally.

### FMM passes

Six standard passes: P2M (leaf Bodies → leaf multipole), M2M (upward: child multipole → parent multipole), M2L (far-field: source multipole → target local expansion), L2L (downward: parent local → child local), L2P (leaf local → Body acceleration), P2P (adjacent leaf Bodies → Body acceleration). M2L covers all far-field; P2P covers all near-field; no dual-tree traversal during M2L.

### Multipole expansions

Full symmetric Cartesian tensor expansions of the Plummer-softened kernel `Φ(r) = -1 / sqrt(r² + ε²)`. All partial derivatives `∂ˣⁱ ∂ʸʲ Φ` for `i + j ≤ p` are tracked — 45 coefficients at p=8. Expansion coefficients are stored as flat `Vec<f64>` arrays indexed by node ID (real-valued, not complex).

### Interaction lists

Precomputed after tree construction. M2L lists are computed via parent-neighbor expansion: children of the parent's 3×3 Moore neighborhood, excluding the node's own adjacent cells. P2P lists identify adjacent leaf neighbor ranges. Storage is fixed-size `[u32; 64]` per node with a separate `[u8]` length array — no heap allocation, no branching in hot M2L loops.

### Parallelism (Rayon)

- P2M, M2L, L2P, P2P: `par_iter()` over nodes/Bodies (no dependencies).
- M2M, L2L: sequential level-by-level, `par_iter()` within each level (parent-child dep across levels).

### Integration

Leapfrog Kick–Drift–Kick. Forces computed at integer time steps. After the final Kick, the first Kick of the next step reuses the just-computed accelerations.

### Δt

Internal stability bound computed from initial conditions (strongest local acceleration). User may provide a cap `--dt`. Actual Δt = min(user_dt, 0.5 × dt_max).

### Error handling

- `thiserror` in galax-core, galax-integrate, galax-init, galax-io for recoverable errors (file I/O, invalid params).
- `anyhow` in galax-cli for ergonomic aggregation.
- No `Result` in hot numerical kernels (P2P, M2L, M2M, L2L, L2P). NaN detection at stage boundaries only.
- Panic allowed in initialization, validation, and post-kernel stage-boundary checks; forbidden inside numerical loops.

### Energy diagnostics

Two-layer system:
- Runtime: FMM-consistent energy computed from the current solver state. Used for continuous drift monitoring. Not physically exact.
- Validation: sampled N² potential energy computed on a random subset. Physics-accurate. Only runs under `--validate`.

### CLI

clap derive. Required: `--init <plummer|disk|uniform>`, `--n <bodies>`, `--steps <N>` or `--t-end <T>`. Optional: `--dt`, `--theta` (0.5), `--softening`, `--p` (8), `--out` (default no disk output), `--snap-every`, `--energy-every`, `--validate`, `--validate-samples` (1000), `--n2-regression` (runtime guard at N ≤ 2000).

### Output format

Binary flat dump: raw bytes of the 7 SoA arrays in fixed order (x, y, vx, vy, mass, ax, ay), preceded by a header with N, timestamp, and version.

### Boundary conditions

Open boundaries. Bodies that leave the simulation domain remain in the Body array but may drift arbitrarily far. No wraparound or reflection.

## Testing Decisions

A good test verifies external behavior and invariants, not implementation details. Tests never mix correctness assertions with performance measurement.

### galax-core
- Unit tests: Morton encode ↔ decode round-trip; tree invariants (contiguous ranges, no overlap, parent covers children); P2M (single Body → analytic moment match); M2M (known translation vector); M2L (one source cell → compare against direct Taylor expansion at target center); L2L (parent → child shift consistency); P2P (pairwise Newton law agreement).
- Integration: Full FMM vs direct N² on N=200–1000, verify far-field + near-field = full direct force.

### galax-integrate
- Unit: KDK on 2-body Kepler orbit (energy bounded, periodicity stable).
- Integration: full pipeline over 10–100 steps at N=500, energy drift within expected FMM truncation error.

### galax-init
- Unit: Plummer sphere generator matches analytic density profile; disk rotation curve matches expected circular velocity.

### galax-io
- Unit: write BodiesSoA → binary buffer → read → data matches exactly (round-trip).

### galax-cli
- Smoke test: `--init uniform --n 100 --steps 5` runs without crash.

Prior art: N/A — first tests in this repo. Follow Rust convention: `#[cfg(test)] mod tests` at bottom of each module file, plus `tests/` integration directory per crate.

## Out of Scope

- GPU acceleration (CPU-only via Rayon)
- 3D simulation (2D only)
- Adaptive time stepping (fixed Δt per stability bound)
- Visualization or rendering (galax-viz is separate; this PRD covers the simulation only)
- Cosmological boundary conditions (periodic, expanding box)
- Relativistic corrections
- Collisional N-body (stellar dynamics — this is collisionless)
- Real-time interactive control
- WebAssembly target
- Python bindings
- Advanced FMM variants (kernel-independent FMM, black-box FMM)

## Further Notes

- Three ADRs exist at `docs/adr/0001-*`, `0002-*`, `0003-*` documenting key architectural decisions. Read them before implementing.
- `CONTEXT.md` at the repo root defines the domain glossary. Use its vocabulary in all code identifiers and documentation.
- The workspace layout is: `galax-core/`, `galax-integrate/`, `galax-init/`, `galax-io/`, `galax-cli/`. Dependencies flow: cli → everything, integrate → core, init → core, io → core. Core has no workspace-internal dependencies.
- Rayon is an unconditional dependency — no feature flag to disable parallelism.
- The softening length ε is estimated from initial conditions (fraction of mean inter-Body spacing) if not user-provided.
