Status: ready-for-agent

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Scaffold the 5-crate Rust workspace (`galax-core`, `galax-integrate`, `galax-init`, `galax-io`, `galax-cli`) with Cargo.toml files, module stubs, and dependencies. In `galax-core`, implement the `BodiesSoA` struct (7 flat `Vec<f64>` arrays: x, y, vx, vy, mass, ax, ay) and the direct N² pairwise force computation using the Plummer-softened kernel. In `galax-cli`, provide a minimal entry point that generates random bodies, computes N² forces once, and prints timing.

This slice cuts through every layer: data layout → force computation → CLI output. No tree, no FMM, no integration.

## Acceptance criteria

- [ ] Workspace builds with `cargo build --workspace`
- [ ] `BodiesSoA` stores 7 arrays with indexed access and length N
- [ ] N² force function computes `1 / sqrt(r² + ε²)` softened kernel
- [ ] Force satisfies Newton's third law (action ≈ -reaction) to machine precision
- [ ] `galax-cli --n 100 --steps 1` runs without error and prints timing
- [ ] Unit tests: pairwise force symmetry, softening at r ≪ ε and r ≫ ε

## Blocked by

None — can start immediately.
