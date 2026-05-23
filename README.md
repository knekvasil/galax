# galax

A 2D gravitational N-body simulation using the Fast Multipole Method,
accelerated by GPU compute (WebGPU / wgpu).

![screenshot](https://img.shields.io/badge/status-active-brightgreen)
![Rust](https://img.shields.io/badge/language-Rust-orange)

## Overview

galax simulates the gravitational interaction of up to hundreds of thousands
of Bodies in 2D using the Fast Multipole Method.  The core algorithm runs
on the GPU via wgpu (Metal, Vulkan, DX12) — and in the browser via WebGPU.

### What makes it fast

- **O(N) FMM** — the Fast Multipole Method replaces the O(N²) all-pairs
  summation with a tree-based approximation that scales linearly with
  the number of bodies
- **GPU compute** — the entire 6-pass FMM pipeline runs as WGSL compute
  shaders, not just a single reduction
- **Adaptive quadtree** — Morton-curve sorted, 2:1 balanced, built in O(N log N)
- **Cartesian Taylor expansions** — the 2D `1/r` gravitational kernel is
  expanded using Cartesian tensors (not complex Laurent series)

## Repository structure

```
galax/                     # Workspace root
├── galax-core/            # FMM, quadtree, BodiesSoA, N² force
├── galax-gpu/             # wgpu GPU backend + WGSL shaders
├── galax-integrate/       # Leapfrog (KDK) time integration
├── galax-init/            # Initial condition generators
├── galax-io/              # Binary snapshot I/O
├── galax-cli/             # Command-line simulation runner
├── galax-viz/             # Interactive native visualization
├── galax-bench/           # Micro-benchmark suite
├── galax-wasm/            # WASM build for in-browser simulation
├── CONTEXT.md             # Domain vocabulary and conventions
├── docs/
│   └── agents/            # Agent workflow documentation
└── .scratch/              # Issue tracking and PRDs
```

### Dependency graph

```
                  galax-core
                 /    |    \
                /     |     \
        galax-init  galax-io  galax-gpu
            |          |       /    \
            |          |      /      \
        galax-integrate <--/        galax-wasm
           /    |    \
          /     |     \
  galax-cli  galax-viz  galax-bench
```

## Quick start

### Run a simulation

```bash
# 100k bodies, Plummer sphere, 200 steps, GPU
cargo run -p galax-cli --release -- --n 100000 --init plummer --gpu --steps 200 --progress
```

### Visualise

```bash
# Interactive GPU-accelerated visualisation
cargo run -p galax-viz --release -- --n 10000 --init disk --gpu
```

Controls: scroll to zoom, left-drag to pan, Escape to exit.

### Run in the browser

```bash
cd galax-wasm
./deploy.sh
npx serve pkg/
# or deploy instantly:
npx surge pkg/ galax-nbody.surge.sh
```

Requires a WebGPU-capable browser (Chrome 113+, Edge 113+, Firefox Nightly).

## Method

### Fast Multipole Method

The FMM decomposes the gravitational force computation into far-field and
near-field components:

1. **P2M** — Each leaf cell computes its multipole moments from its Bodies
2. **M2M** — Multipole moments propagate upward through the tree
3. **M2L** — Well-separated cells exchange multipole → local translations
4. **L2L** — Local expansions propagate downward through the tree
5. **L2P** — Each Body evaluates the local expansion at its position
6. **P2P** — Bodies in adjacent leaf cells interact directly

At multipole order `p = 4`, the RMS force error is ~0.26% relative to N².
Throughput is O(N) for FMM vs O(N²) for direct summation.

### Plummer softening

The softened gravitational kernel `Φ(r) = -1/√(r² + ε²)` is expanded
directly in the FMM — softening is not applied as a post-correction.

### Integration

Time integration uses the second-order symplectic Leapfrog (Kick-Drift-Kick)
scheme, which conserves energy to machine precision over long timescales
(energy drift < 1% over 100 orbital periods in the two-body test).

## Domain vocabulary

See `CONTEXT.md` for the project's domain glossary. Key terms:

- **Body** — a point mass with position, velocity, and mass (not "particle")
- **Gravitational Interaction** — pairwise force between two Bodies
- **Softening (Plummer softening)** — modified force law avoiding singularities
- **Quadtree Cell** — spatial decomposition tree node
- **Cartesian Multipole Expansion** — Taylor expansion of the potential

## Performance targets

| Configuration | Expected perf |
|---|---|
| 100k bodies, GPU (M1 Mac native) | ~60 fps |
| 100k bodies, WebGPU (M1 Mac, Chrome) | ~30–60 fps |
| 10k bodies, WebGPU (recent phone) | ~30 fps |
| 100k bodies, CPU FMM (8 cores) | ~5–10 steps/s |
| 100k bodies, CPU N² | ~0.1 steps/s |

## License

MIT
