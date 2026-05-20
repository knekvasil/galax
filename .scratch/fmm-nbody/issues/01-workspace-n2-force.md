Status: completed

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Scaffold the 5-crate Rust workspace (`galax-core`, `galax-integrate`, `galax-init`, `galax-io`, `galax-cli`) with Cargo.toml files, module stubs, and dependencies. In `galax-core`, implement the `BodiesSoA` struct (7 flat `Vec<f64>` arrays: x, y, vx, vy, mass, ax, ay) and the direct N² pairwise force computation using the Plummer-softened kernel. In `galax-cli`, provide a minimal entry point that generates random bodies, computes N² forces once, and prints timing.

## Acceptance criteria

- [x] Workspace builds with `cargo build --workspace`
- [x] `BodiesSoA` stores 7 arrays with indexed access and length N
- [x] N² force function computes `1 / sqrt(r² + ε²)` softened kernel
- [x] Force satisfies Newton's third law (action ≈ -reaction) to machine precision
- [x] `galax-cli --n 100 --steps 1` runs without error and prints timing
- [x] Unit tests: pairwise force symmetry, softening at r ≪ ε and r ≫ ε

## Completion notes

- Workspace with 5 crates created: galax-core, galax-integrate, galax-init, galax-io, galax-cli
- BodiesSoA with 7 public Vec<f64> fields (x, y, vx, vy, mass, ax, ay)
- compute_n2_force as free function in galax-core
- 4 unit tests: force symmetry, known distance, zero separation, linear softening regime

## Blocked by

None — can start immediately.
