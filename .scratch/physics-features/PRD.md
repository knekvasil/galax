Status: ready-for-agent

# PRD: Physics feature roadmap for galax

## Problem Statement

galax currently simulates collisionless stellar dynamics using the Fast Multipole Method with Plummer softening and Leapfrog integration. This is sufficient for many galactic dynamics studies, but several nearby physics features — adaptive timestepping, multiple particle species, sink particles, higher-order integrators, tidal diagnostics, and sub-grid models — are not yet implemented. A developer picking up this codebase has no documented map of which features are feasible, what they cost in performance, and which modules they touch.

## Solution

A catalog of physics features, each scoped with implementation decisions, performance impact estimates, module boundaries, and testing strategy. This document serves as a roadmap — not a commitment to build, but a shared reference for deciding what to build next. Features are ordered by effort (easiest first) and each is self-contained: any one can be implemented without the others.

## User Stories

1. As a developer, I want to understand the performance impact of each possible physics feature before committing to it, so that I can make informed trade-offs between richness and speed.

2. As a developer, I want each feature to be independently implementable, so that I can ship value incrementally without a multi-month refactor.

3. As a developer, I want to know which modules each feature touches, so that I can estimate scope and avoid surprise rewrites.

4. As a developer, I want a testing strategy for each feature, so that I can verify correctness without depending on features not yet built.

5. As a user, I want adaptive timestepping so that close encounters are resolved accurately without wasting cycles on quiet regions.

6. As a user, I want multiple particle species (stars, dark matter, gas with different mass scales) so that I can simulate mixed-component systems.

7. As a user, I want per-species softening so that dense star clusters are not over-softened by the diffuse DM halo value.

8. As a user, I want sink particles so that accreting bodies (black holes, star-forming clumps) grow dynamically during the simulation.

9. As a user, I want higher-order symplectic integrators so that energy conservation improves without reducing the timestep.

10. As a user, I want galaxy-galaxy merger initial conditions so that I can study tidal interactions and stripping.

11. As a user, I want the tidal tensor extracted from the FMM expansions so that I can identify tidal disruption and Roche lobe overflow.

12. As a user, I want dynamical friction on massive bodies so that supermassive black holes spiral to galactic centers realistically.

13. As a user, I want a friends-of-friends halo finder so that I can identify bound structures in the simulation output.

14. As a developer, I want SPH gas dynamics so that I can study galaxy formation with star formation and feedback.

15. As a developer, I want 3D simulation capability so that the codebase is not limited to 2D physics.

## Implementation Decisions

### Feature 1: Adaptive timestepping

Each Body gets an individual timestep `dt_i = η × sqrt(ε / |a_i|)` capped by a global maximum `dt_max`. Bodies are grouped by timestep into power-of-two bins (`dt_max / 2^k`). The integration uses a block-step scheme: all Bodies drift to the next synchronization point, but only the active group kicks.

**What it touches:**
- `galax-integrate`: new `AdaptiveIntegrator` module. New function `adaptive_leapfrog_step` that schedules active groups.
- `galax-core`: may expose `compute_dt_for_body(body_i, bodies, epsilon)` for per-Body stability bound.
- `galax-gpu`: GPU FMM computes all accelerations every step anyway — no change.
- `galax-viz` / `galax-wasm`: no rendering changes. HUD should show `min(dt_i)`.

**Performance impact:** CPU-side scheduling overhead of ~1-3%. GPU unchanged. The real gain is accuracy: close binaries no longer force a tiny global dt for all Bodies.

**Testing:**
- Energy drift for a near-Kepler pair should be ≤ 1% over 100 orbits (same threshold as global-dt test).
- Single-body drift with zero acceleration should take minimum dt steps.
- Verify that inactive bodies do not have their positions/velocities modified.

### Feature 2: Multiple particle species

A `species` field (`u8` tag) added to `BodiesSoA`. The FMM kernel is unchanged — it only reads `mass`, `x`, `y`. Species is used by post-processing (FoF, diagnostics) and by future features (SPH, star formation).

**What it touches:**
- `galax-core`: `BodiesSoA` gains a `species: Vec<u8>` field. All constructors, copy/clone impls, and the binary I/O format updated.
- `galax-init`: init generators assign a default species (all stars, or separate DM/star setup).
- `galax-io`: snapshot format extended with an 8th array (one byte per Body, padded to alignment).
- `galax-cli`: `--species` option for mixed initialization (e.g. `--dark-matter-fraction 0.9`).
- `galax-gpu`: not touched (kernel doesn't read species).
- `galax-viz` / `galax-wasm`: optional — render different species as different colors.

**Performance impact:** <0.5%. The extra `Vec<u8>` is never read in hot loops.

**Testing:**
- Round-trip `BodiesSoA` → binary → `BodiesSoA` preserves species tags.
- Mixed-species simulation conserves total momentum and energy (species tags don't affect dynamics).
- `galax-gpu` crosscheck passes with multiple species present (verifies kernel doesn't accidentally read the field).

### Feature 3: Per-species softening

Replace global `softening: f64` with per-species `epsilon: Vec<f64>` (length = number of species). For a Body pair `(i, j)`, the effective softening is `(eps[species[i]] + eps[species[j]]) / 2`.

**What it touches:**
- `galax-core`: `compute_n2_force` gains per-species epsilon parameter. `compute_fmm_force` passes per-species epsilon through to P2P. M2L expansion uses the user-specified epsilon (the far-field softening is a single representative value, typically the DM epsilon).
- `galax-gpu`: P2P shader reads a new `species_eps` storage buffer. The shared `sim_params` struct adds a pointer. M2L shader unchanged (still uses global eps from uniform buffer).
- `galax-cli` / `galax-viz`: `--softening` becomes `--softening-dm --softening-star`. Default: both equal.

**Performance impact:** P2P shader reads one extra buffer per pair. P2P is ~10% of step time. Impact: ~3-5% on total step time.

**Testing:**
- Two-species setup with DM eps >> star eps: dense star cluster should remain bound, diffuse DM should show larger softening effects.
- N² crosscheck: P2P-only test with per-species eps matches CPU reference with same per-species rule.

### Feature 4: Sink particles

A sink Body has a radius `r_sink`, an accretion rate, and a flag `is_sink`. When a non-sink Body enters `r_sink`, its mass is added to the sink and the Body is removed from the simulation (replaced by compaction or a sentinel).

**What it touches:**
- `galax-core`: `BodiesSoA` gains `sink_flag: Vec<bool>` and `sink_radius: Vec<f64>`. New free function `accrete_sinks(bodies, tree, sink_indices)` called after drift but before force computation.
- `galax-core`: tree must be rebuilt when Bodies are removed (masses change). If sink removes many Bodies per step, tree rebuild frequency increases.
- `galax-gpu`: sink accretion happens on CPU after drift, before the next FMM step. No GPU changes.
- `galax-init`: a new generator `galaxy_with_bh` that places a central SMBH.

**Performance impact:** ~2% if masses change gradually. More if sinks remove many Bodies per step (forces tree rebuild). Sink accretion logic is O(N_sink × N) naively; can be accelerated with the quadtree.

**Testing:**
- Single sink with one test Body on a radial orbit: verify the test Body is absorbed within r_sink.
- Energy + momentum conservation during accretion (mass is transferred, not lost).
- Sink mass growth rate matches expected Bondi-Hoyle formula for known density + velocity.

### Feature 5: Higher-order symplectic integrators

Forest-Ruth (4th order) and Yoshida (6th order) as drop-in replacements for Leapfrog. These are compositions of the same Kick-Drift operators with different coefficients.

**What it touches:**
- `galax-integrate`: new module `composite_integrators` with `forest_ruth_kdk` and `yoshida_kdk`. The general form is a sequence of `(kick_coeff, drift_coeff)` pairs.
- `galax-core`: no changes (force function is a parameter).
- `galax-gpu`: `leapfrog_kdk_gpu` gets a higher-order counterpart `forest_ruth_kdk_gpu` that calls `step()` + `read_accelerations()` multiple times per macro-step.
- `galax-cli`: `--integrator leapfrog|forest-ruth|yoshida`.

**Performance impact:** Forest-Ruth = 3 force evaluations per step (3× Leapfrog cost). Yoshida = 7 evaluations. However, for the same energy error, a higher-order integrator can take a much larger dt. Net: usually faster for a given accuracy target.

**Testing:**
- Kepler 2-body over 100 orbits: energy drift should decrease by ~ (dt)² for Forest-Ruth vs Leapfrog.
- Period error should match the expected order.
- Compare against Leapfrog with same dt and verify Forest-Ruth has lower drift (not measuring absolute accuracy, just relative order convergence).

### Feature 6: Galaxy-galaxy merger ICs

A CLI generator that creates two Plummer or disk models, displaces them in position and velocity, and sets them orbiting their mutual barycenter.

**What it touches:**
- `galax-init`: new function `merger_ics(primary: BodiesSoA, secondary: BodiesSoA, separation: f64, impact_parameter: f64) → BodiesSoA` that superposes the two body arrays and adjusts velocities for orbital motion.
- `galax-cli`: `--merger --primary plummer,10000 --secondary disk,5000 --sep 50`.
- `galax-viz` / `galax-wasm`: no changes.

**Performance impact:** 0% (same simulation as a single system).

**Testing:**
- Merger IC at zero separation equals the primary alone.
- Conservation of center-of-mass velocity (should match the Keplerian barycentric motion).
- Visual regression: known test pattern (two Plummer spheres merging, check time to first pericenter).

### Feature 7: Tidal tensor

The FMM Cartesian expansion coefficients are the derivatives `∂ˣⁱ∂ʸʲΦ`. The tidal tensor `T_ab = ∂²Φ / ∂x_a ∂x_b` is the i+j=2 block (p≥2 terms). Extract it from the local expansion at each Body during L2P.

**What it touches:**
- `galax-core`: `compute_fmm_force` writes tidal tensor to new output arrays `tidal_xx, tidal_xy, tidal_yy` on `BodiesSoA`. Note: FMM local expansions are per-leaf, not per-Body. The tidal tensor at each Body requires evaluating the Leaf local expansion gradient at the Body's position — already computed during L2P for acceleration (first derivative); the second derivative is the next coefficient in the same Taylor series.
- `galax-gpu`: L2P shader gains an optional output buffer for the 3 tidal components per Body.
- `galax-cli`: `--tidal` flag.

**Performance impact:** ~0%. The derivatives are already computed — just need to write them to an output buffer instead of discarding.

**Testing:**
- Point-mass potential: tidal tensor should be diagonal with `-1/r³` and `+1/2r³` for the x and y eigenvalues (traceless).
- Hollow sphere: tidal tensor should be zero inside the cavity.

### Feature 8: SMBH dynamics / dynamical friction

A sub-grid dynamical friction force `F_df = -4π G² M² ρ lnΛ / |v|³ × v` (Chandrasekhar formula) applied to SMBH particles. Requires local density estimate at each SMBH position.

**What it touches:**
- `galax-core`: new module `dynamical_friction`. Function `apply_df(bodies: &mut BodiesSoA, tree: &Tree, bh_indices: &[usize], G: f64, coulomb_log: f64, dt: f64)` that estimates local density from the quadtree (leaf mass / leaf area), computes the friction force, and applies an impulse to the BH's velocity. Called between drift and kick in the integrator.
- `galax-init`: `galaxy_with_bh` generator (same as Feature 4) places SMBH at center with zero initial velocity.
- `galax-gpu`: friction computed on CPU. No shader changes.

**Performance impact:** O(N_bh × tree_depth) density lookups. For N_bh ≤ 10 and the quadtree: ~1-2% overhead.

**Testing:**
- SMBH in a uniform Plummer sphere: verify sinking timescale matches Chandrasekhar prediction (order-of-magnitude).
- Without dynamical friction, SMBH on a circular orbit should stay on that orbit (energy conserved).
- With friction, SMBH should spiral in and circularize.

### Feature 9: Friends-of-friends halo finder

Cluster Bodies by linking length `b`. Two Bodies are in the same group if their separation ≤ b. The quadtree speeds up neighbor queries: for each unvisited leaf, walk nearby leaves and flood-fill.

**What it touches:**
- `galax-core`: new module `halo_finder`. Function `friends_of_friends(bodies, tree, linking_length) → Vec<Vec<usize>>` returns groups of body indices. Also produces summary statistics: `HaloCatalog` struct with per-halo mass, center-of-mass, velocity dispersion, spin.
- `galax-cli`: `--fof` flag. Outputs halo catalog as JSON or text.
- `galax-viz` / `galax-wasm`: optional overlay — color Bodies by halo membership.

**Performance impact:** 0% (post-processing, not per-step). Runs once at end or at select snapshots.

**Testing:**
- Two isolated Plummer spheres separated by >> linking length: should identify exactly two halos.
- Uniform distribution with linking length >> domain size: should identify one halo containing all Bodies.
- Halo catalog output has correct total mass (sum of member masses == total system mass).

### Feature 10: SPH gas dynamics

Gas particles carry density `ρ`, internal energy `u`, and smoothing length `h`. Forces have a pressure component `P = (γ-1) ρ u` plus artificial viscosity. The quadtree is used for neighbor search (find particles within 2h of each gas particle).

**What it touches:**
- `galax-core`: new SPH module. New `BodiesSph` struct or extend `BodiesSoA` with optional gas arrays (density, internal_energy, smooth_length, gas_mask). New free functions: `compute_density`, `compute_pressure`, `compute_sph_forces` (pressure gradient + artificial viscosity). These are O(N_gas × N_neighbors) — neighbor search via tree walk.
- `galax-gpu`: new SPH compute pipelines. The tree is already on GPU; a density pass queries neighbor counts, then a force pass computes SPH accelerations. WGSL shaders for density and hydro forces.
- `galax-init`: gas initial conditions (uniform density sphere with thermal energy, or rotating disk with Toomre Q profile).
- `galax-integrate`: combined gravity + hydro integration. Operator splitting: hydro forces + gravity in the same kick.
- `galax-cli`: `--init gas-cloud` for pure SPH, or `--init galaxy-gas` for stars + gas.

**Performance impact:** ~30-50% slower. Each gas particle needs neighbor search (tree walk) plus density/hydro evaluations. The P2P pass becomes heavier (hydro forces per neighbor pair). The density pass adds a new GPU dispatch. Total step time roughly doubles for 50% gas fraction.

**Testing:**
- Sod shock tube (1D): density, velocity, pressure profiles should match analytic solution.
- Hydrostatic sphere: gas should stay in equilibrium (no bulk motion).
- Energy conservation: total (kinetic + thermal + potential) should be conserved to FMM precision in adiabatic runs.
- Verify gas particles do not penetrate each other (artificial viscosity prevents pairing instability).

### Feature 11: 3D simulation

Replace the 2D quadtree with a 3D octree. Morton keys become 3D interleaving (3× 21-bit coordinates → 63-bit keys). Cartesian expansions grow from `(p+1)(p+2)/2` to `(p+1)(p+2)(p+3)/6` coefficients per node (~120 vs 45 at p=4, ~286 vs 55 at p=8). M2L considers the 3×3×3 = 27 neighbor cells instead of 3×3 = 9. Rendering requires 3D projection (orthographic or perspective).

**What it touches:**
- `galax-core`: generalization of `Tree` to support both 2D and 3D via a `const DIM: usize` generic parameter, or a parallel 3D implementation. Morton encoding functions for 3D. New `BodiesSoA` has `z` and `vz` arrays. New `Cell3D` and `Node3D` structs. All FMM passes: P2M accumulates 3D multipole moments, M2M shifts in 3D, M2L has 27 neighbors, L2L shifts in 3D, L2P evaluates 3D gradient, P2P computes 3D pairwise forces.
- `galax-gpu`: all 6 WGSL shaders rewritten for 3D. The octree data layout and interaction list format changes. Buffer sizes increase (more cells, larger expansion arrays, more interaction indices).
- `galax-init`: plummer, disk, uniform generators gain a z-dimension. Uniform naturally generalizes. Plummer sampling remains the same (spherically symmetric). Disk extends with vertical exponential profile.
- `galax-integrate`: integrators work with 3D arrays. No conceptual change.
- `galax-viz` / `galax-wasm`: 3D rendering — orthographic projection of (x,y,z) to screen with optional rotation. Or step upgrade: at minimum, project z onto the screen plane (view from above).

**Performance impact:** ~5-10× slower for same N. Factors: 27× neighbor count in M2L vs 9×, 2.5-5× larger expansions, 3× more position/velocity data, 3D tree ~2× more nodes. Estimating: 100k 2D → 10-20k 3D at same frame rate.

**Testing:**
- 3D FMM vs direct N² on N=500: verify force error matches the same RMS tolerance as the 2D test suite.
- 3D Plummer sphere: radial density profile should match analytic.
- Kepler orbit in 3D: orbital plane should remain constant (angular momentum conserved).

### Feature 12: Cosmological expansion

Comoving coordinates with scale factor `a(t)`. Equations: `dx/dt = v_comoving`, `dv/dt = -2H(t) v_comoving + g_comoving` where `H = da/dt / a`. The gravitational force is computed in comoving coordinates with density contrast (subtract mean density). No periodic BCs for MVP.

**What it touches:**
- `galax-core`: force computation optionally subtracts the mean density term (a correction to the Poisson equation in comoving coordinates). New function `apply_hubble_drag(bodies, H, dt)`.
- `galax-integrate`: integrator accepts scale factor and computes `H(t)`. The Kepler solver verifies the Friedmann equation.
- `galax-gpu`: shell expansion and Hubble drag applied on CPU. No shader changes if mean density subtraction is handled analytically.

**Performance impact:** ~0% (scale factor integration is a scalar ODE, cheap).

**Testing:**
- Single-body in an expanding universe: comoving position should remain constant (canonical test — no peculiar velocity → no acceleration → position unchanged).
- Two-body problem in expanding background: physical separation should follow the background expansion for small masses.

## Testing Decisions

A good test verifies external behavior: energy/momentum conservation, known analytic solutions, and regression against N² reference. Tests never mix correctness assertions with performance measurement.

### Per-feature test module
Each feature's tests live in the module or crate it modifies, following the existing pattern `#[cfg(test)] mod tests` at the bottom of each module file.

### Prior art
- `galax-core`: unit tests for math invariants (Morton encode/decode, kernel symmetry), integration tests for full FMM vs N².
- `galax-gpu`: `test_gpu_full_fmm_vs_cpu_fmm` compares GPU vs CPU FMM.
- `galax-integrate`: `test_kepler_one_orbit` verifies energy drift < 1% over 1 Kepler orbit.

Each new feature test follows the same pattern: run the feature, compare against a known reference (analytic or N²), assert tolerance.

## Out of Scope

- Non-gravitational physics (magnetic fields, radiation transport, cosmic rays)
- General relativistic corrections (post-Newtonian terms, gravitational waves)
- Mesh-based methods (grid hydro, adaptive mesh refinement)
- Machine-learned sub-grid models
- Real-time interactive controls beyond existing camera/UI
- Python or other language bindings
- GPU rendering (stays CPU-side pixel buffer)

## Further Notes

- All features are listed in approximate effort order (easiest first). A developer could start with Feature 1 (adaptive timestepping) as a self-contained week-long project.
- Features 10 (SPH) and 11 (3D) are structural rewrites of the core FMM engine. They should be the last attempted, not the first.
- None of these features require WASM-specific changes. The GPU compute (WGSL) and CPU-side (Rust) are the same — the web deployment path is orthogonal to physics richness.
- ADR-0004 (f32 GPU compute) and ADR-0005 (wgpu backend) are unaffected by any feature here. All GPU-side features use f32 and the existing wgpu pipeline.
- If a future 3D implementation is planned, Features 2-9 should ensure their data structures and APIs generalize to `const DIM: usize` rather than hardcoding 2D.
